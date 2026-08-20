/*
 * diad-lock-auth — PAM-backed password verification for the diadOS web
 * lockscreen (MASTER FIX S2-style validated helper).
 *
 * The web lock page POSTs the password to the daemon; the daemon runs this
 * helper via sudo (NOPASSWD, pinned exact path in /etc/sudoers.d/diad —
 * no arguments are accepted). The password travels on STDIN only, never on
 * the command line and never in logs.
 *
 * The account to authenticate is taken from SUDO_USER (set by sudo), falling
 * back to the USER environment variable — so the helper never needs to know
 * who is logging in and cannot be pointed at an arbitrary account via argv.
 *
 * Exit codes:
 *   0  — password verified, account usable (auth + acct_mgmt both OK)
 *   1  — verification failed (bad password, locked/expired account, error)
 *
 * Build (host, at ISO build time):  gcc -O2 -Wall -Wextra -Werror -o ... -lpam
 */

#include <security/pam_appl.h>
#include <security/pam_misc.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

static char g_password[256]; /* one NUL-terminated password line */

/* Read exactly one line of password from stdin (no echo). */
static int read_password(void)
{
    if (fgets(g_password, sizeof(g_password), stdin) == NULL)
        return 0;
    g_password[strcspn(g_password, "\r\n")] = '\0';
    /* match swaylock's ignore-empty-password: an empty attempt never unlocks */
    return (g_password[0] != '\0');
}

static int pam_conversation(int num_msg, const struct pam_message **msg,
                            struct pam_response **resp, void *appdata_ptr)
{
    (void)appdata_ptr;
    struct pam_response *reply =
        calloc((size_t)num_msg, sizeof(struct pam_response));
    if (reply == NULL)
        return PAM_BUF_ERR;

    for (int i = 0; i < num_msg; i++) {
        switch (msg[i]->msg_style) {
        case PAM_PROMPT_ECHO_OFF:
            reply[i].resp = strdup(g_password);
            break;
        case PAM_PROMPT_ECHO_ON:
            reply[i].resp = strdup("");
            break;
        case PAM_ERROR_MSG:
        case PAM_TEXT_INFO:
            reply[i].resp = NULL;
            break;
        default:
            free(reply);
            return PAM_CONV_ERR;
        }
        if (msg[i]->msg_style != PAM_ERROR_MSG &&
            msg[i]->msg_style != PAM_TEXT_INFO && reply[i].resp == NULL) {
            free(reply);
            return PAM_BUF_ERR;
        }
    }
    *resp = reply;
    return PAM_SUCCESS;
}

int main(void)
{
    static const struct pam_conv conv = {
        .conv = pam_conversation,
        .appdata_ptr = NULL,
    };
    const char *user = getenv("SUDO_USER");
    if (user == NULL || user[0] == '\0')
        user = getenv("USER");
    if (user == NULL || user[0] == '\0')
        return 1;

    if (!read_password())
        return 1; /* empty stdin or empty password: never unlocks */

    pam_handle_t *pamh = NULL;
    int rc = pam_start("system-auth", user, &conv, &pamh);
    if (rc != PAM_SUCCESS)
        return 1;

    rc = pam_authenticate(pamh, 0);
    if (rc == PAM_SUCCESS)
        rc = pam_acct_mgmt(pamh, 0);

    pam_end(pamh, rc);
    return (rc == PAM_SUCCESS) ? 0 : 1;
}
