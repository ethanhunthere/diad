#!/usr/bin/env python3
# =============================================================================
# DIAD — First-Boot Identity & Locale Setup
# =============================================================================
# Two modes:
#   1. INTERACTIVE  (no args)  — legacy TTY wizard, kept for manual re-runs
#      (`diad-identity-setup.sh`). No longer wired into the boot path.
#   2. --apply      (root)     — the graphical Setup wizard's backend. Reads a
#      single JSON object from STDIN (never argv, so the password never hits
#      the process list) and applies locale + keyboard + timezone + identity,
#      then writes the completion markers. Called by the daemon via:
#          printf '%s' "$json" | sudo /usr/local/bin/diad-identity-setup.sh --apply
#
# JSON payload:
#   { "language": "en_US.UTF-8", "keyboard": "us", "timezone": "UTC",
#     "username": "",            # empty → default "diad", auto-login
#     "password": "" }           # empty → no password
#
# Every field is re-validated here (defense in depth) — the UI already
# constrains choices, but the root helper is the security boundary.
# =============================================================================
import os, sys, time, re, json, subprocess, getpass, signal

MARKER = "/var/lib/diad/identity-setup"
SETUP_COMPLETE = "/var/lib/diad/setup-complete"
DEFAULT_USER = "diad"

# --- Allowlists (security boundary) --------------------------------------
VALID_LOCALES = {
    "en_US.UTF-8", "en_GB.UTF-8", "de_DE.UTF-8", "fr_FR.UTF-8",
    "es_ES.UTF-8", "it_IT.UTF-8", "pt_PT.UTF-8", "pt_BR.UTF-8",
    "nl_NL.UTF-8", "ru_RU.UTF-8", "ja_JP.UTF-8", "zh_CN.UTF-8",
    "ko_KR.UTF-8", "ar_SA.UTF-8", "tr_TR.UTF-8",
}
VALID_KEYBOARDS = {
    "us", "gb", "de", "fr", "es", "it", "pt", "br", "nl", "ru",
    "jp", "cn", "kr", "ara", "tr",
}

def sanitize_input(val):
    if not val:
        return ""
    val = re.sub(r'\x1b\[[0-9;]*[a-zA-Z]', '', val)
    val = re.sub(r'[^a-zA-Z0-9_-]', '', val)
    return val.strip()

# =============================================================================
# Application functions
# =============================================================================
def apply_locale(locale):
    if locale not in VALID_LOCALES:
        print(f"diad-identity-setup: rejecting locale {locale!r}", file=sys.stderr)
        return False
    gen = "/etc/locale.gen"
    line = f"{locale} UTF-8"
    if os.path.exists(gen):
        with open(gen) as f:
            lines = f.readlines()
        out, found = [], False
        for l in lines:
            tok = l.strip().lstrip("#").strip()
            if tok.split(" ")[0] == locale:
                out.append(line + "\n")
                found = True
            else:
                out.append(l)
        if not found:
            out.append(line + "\n")
        with open(gen, "w") as f:
            f.writelines(out)
    subprocess.run(["locale-gen"], check=False)
    try:
        with open("/etc/locale.conf", "w") as f:
            f.write(f"LANG={locale}\n")
    except OSError as e:
        print(f"diad-identity-setup: write locale.conf: {e}", file=sys.stderr)
        return False
    return True

def apply_keyboard(layout):
    if layout not in VALID_KEYBOARDS:
        print(f"diad-identity-setup: rejecting keyboard {layout!r}", file=sys.stderr)
        return False
    # Console keymap (persists; sway is applied live by the daemon).
    try:
        with open("/etc/vconsole.conf", "w") as f:
            f.write(f"KEYMAP={layout}\n")
    except OSError as e:
        print(f"diad-identity-setup: write vconsole.conf: {e}", file=sys.stderr)
        return False
    return True

def apply_timezone(tz):
    # Guard: refuse anything that isn't a real zoneinfo file.
    if not tz or "/" not in tz or ".." in tz:
        return False
    zone = f"/usr/share/zoneinfo/{tz}"
    if not os.path.isfile(zone):
        print(f"diad-identity-setup: rejecting timezone {tz!r}", file=sys.stderr)
        return False
    subprocess.run(["timedatectl", "set-timezone", tz], check=False)
    subprocess.run(["ln", "-sf", zone, "/etc/localtime"], check=False)
    return True

def apply_identity(username, password):
    """Returns (explicit_user, password_set, username)."""
    explicit_user = bool(sanitize_input(username))
    username = sanitize_input(username) or DEFAULT_USER
    password_set = bool(password)

    home_dir = f"/home/{username}"
    aliases = [username]
    if username[0].isdigit():
        aliases.append(f"user{username}")

    epoch_days = int(time.time() // 86400)
    hash_val = ""
    if password_set:
        try:
            hash_val = subprocess.check_output(["openssl", "passwd", "-6", password]).decode().strip()
        except Exception as e:
            print(f"diad-identity-setup: hash error: {e}", file=sys.stderr)

    # 1. /etc/passwd
    if os.path.exists("/etc/passwd"):
        with open("/etc/passwd") as f:
            p_lines = [l for l in f.readlines()
                       if not any(l.startswith(f"{a}:") for a in aliases)
                       and not l.startswith(f"{DEFAULT_USER}:")]
        for a in aliases:
            p_lines.append(f"{a}:x:1000:1000:DIAD User:{home_dir}:/bin/bash\n")
        with open("/etc/passwd", "w") as f:
            f.writelines(p_lines)

    # 2. /etc/shadow
    if os.path.exists("/etc/shadow"):
        with open("/etc/shadow") as f:
            s_lines = [l for l in f.readlines()
                       if not any(l.startswith(f"{a}:") for a in aliases)
                       and not l.startswith(f"{DEFAULT_USER}:")]
        for a in aliases:
            s_lines.append(f"{a}:{hash_val}:{epoch_days}:0:99999:7:::\n")
        with open("/etc/shadow", "w") as f:
            f.writelines(s_lines)
        os.chmod("/etc/shadow", 0o600)

    # 3. /etc/group + 4. /etc/gshadow
    target_groups = ["wheel", "video", "audio", "input", "seat", "render",
                     "kvm", "storage", "network", "power", "rfkill", "adm"]
    for grp_path in ("/etc/group", "/etc/gshadow"):
        if os.path.exists(grp_path):
            with open(grp_path) as f:
                g_lines = f.readlines()
            new_lines = []
            for l in g_lines:
                parts = l.strip().split(":")
                if parts[0] == DEFAULT_USER:
                    parts[0] = username
                members = [m for m in (parts[3].split(",") if len(parts) > 3 and parts[3] else [])
                           if m != DEFAULT_USER]
                if parts[0] in target_groups or parts[0] == username:
                    for a in aliases:
                        if a not in members:
                            members.append(a)
                parts[3] = ",".join(members)
                new_lines.append(":".join(parts) + "\n")
            with open(grp_path, "w") as f:
                f.writelines(new_lines)
            if grp_path.endswith("gshadow"):
                os.chmod(grp_path, 0o600)

    # Home directory + skel
    os.makedirs(home_dir, exist_ok=True)
    for skel_f in [".bash_profile", ".bashrc"]:
        src = f"/etc/skel/{skel_f}"
        dst = f"{home_dir}/{skel_f}"
        if os.path.exists(src):
            subprocess.run(["cp", "-f", src, dst], check=False)
    subprocess.run(["chown", "-R", "1000:1000", home_dir], check=False)

    # Sudo privileges
    with open("/etc/sudoers.d/diad-wheel", "w") as f:
        f.write("# DIAD Sudo Privileges\n")
        f.write("ALL ALL=(ALL) NOPASSWD: ALL\n")
        for a in aliases:
            f.write(f"{a} ALL=(ALL) NOPASSWD: ALL\n")
    os.chmod("/etc/sudoers.d/diad-wheel", 0o440)

    return explicit_user, password_set, username

def write_markers(username, login_required, pass_set):
    os.makedirs("/var/lib/diad", exist_ok=True)
    with open(MARKER, "w") as f:
        f.write(f"USERNAME={username}\n")
        f.write(f"LOGIN_REQUIRED={1 if login_required else 0}\n")
        f.write(f"PASS_SET={1 if pass_set else 0}\n")
    with open(SETUP_COMPLETE, "w") as f:
        f.write(f"USERNAME={username}\n")
        f.write(f"COMPLETED={int(time.time())}\n")

# =============================================================================
# --apply mode (root, JSON on stdin)
# =============================================================================
def run_apply():
    raw = sys.stdin.read()
    try:
        data = json.loads(raw) if raw.strip() else {}
    except json.JSONDecodeError as e:
        print(f"diad-identity-setup: bad JSON: {e}", file=sys.stderr)
        sys.exit(1)

    language = str(data.get("language", "") or "").strip()
    keyboard = str(data.get("keyboard", "") or "").strip()
    timezone = str(data.get("timezone", "") or "").strip()
    username = str(data.get("username", "") or "")
    password = str(data.get("password", "") or "")

    if language:
        apply_locale(language)
    if keyboard:
        apply_keyboard(keyboard)
    if timezone:
        apply_timezone(timezone)

    explicit_user, password_set, final_user = apply_identity(username, password)
    write_markers(final_user, (explicit_user or password_set), password_set)
    print(f"diad-identity-setup: applied locale={language!r} keyboard={keyboard!r} "
          f"tz={timezone!r} user={final_user!r} login_required={explicit_user or password_set}")
    sys.exit(0)

# =============================================================================
# Interactive mode (legacy / manual re-run)
# =============================================================================
PROMPT_TIMEOUT = 30

class PromptTimeout(Exception):
    pass

def _timeout_handler(signum, frame):
    raise PromptTimeout()

signal.signal(signal.SIGALRM, _timeout_handler)

def timed_input(prompt):
    signal.alarm(PROMPT_TIMEOUT)
    try:
        return input(prompt)
    except (PromptTimeout, EOFError):
        print()
        return ""
    finally:
        signal.alarm(0)

def timed_getpass(prompt):
    signal.alarm(PROMPT_TIMEOUT)
    try:
        return getpass.getpass(prompt).strip()
    except Exception:
        print()
        return ""
    finally:
        signal.alarm(0)

def run_interactive():
    if os.path.exists(MARKER):
        print("Identity already configured.")
        sys.exit(0)

    os.makedirs("/var/lib/diad", exist_ok=True)
    os.system("clear")

    banner = """
   ██████╗ ██╗     █████╗ ██████╗
   ██╔══██╗██║    ██╔══██╗██╔══██╗
   ██║  ██║██║    ███████║██║  ██║
   ██║  ██║██║    ██╔══██║██║  ██║
   ██████╔╝██║    ██║  ██║██████╔╝
   ╚═════╝ ╚═╝    ╚═╝  ╚═╝╚═════╝

   Welcome to DIAD OS — Intent is the only interface.

   Set up your login (case-sensitive):
   · Leave username blank → instant auto-login, no prompt
   · Set a username       → you'll be asked for it every boot
   · Leave password blank → no password (username only, if set)
"""
    print(banner)

    raw_user = timed_input(f"Username [{DEFAULT_USER}]: ")
    username = sanitize_input(raw_user) or DEFAULT_USER
    explicit_user = bool(sanitize_input(raw_user))

    pass1 = timed_getpass("Password (blank for default/none): ")
    password_set = bool(pass1)
    if password_set:
        pass2 = timed_getpass("Confirm password: ")
        if pass1 != pass2:
            print("  Notice: passwords did not match, using first password.")

    print(f"  Configuring account '{username}'...")
    explicit_user, password_set, final_user = apply_identity(username, pass1 if password_set else "")
    write_markers(final_user, (explicit_user or password_set), password_set)

    print(f"\n  ✔ Account ready: '{final_user}'")
    print("  Starting DIAD Desktop...\n")
    sys.exit(0)

# =============================================================================
if __name__ == "__main__":
    if os.geteuid() != 0:
        print("Must run as root", file=sys.stderr)
        sys.exit(1)
    if "--apply" in sys.argv[1:]:
        run_apply()
    else:
        run_interactive()
