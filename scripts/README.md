# `scripts/` — The Test Lab

These scripts **test, audit, and validate** DIAD — run it in a virtual machine,
check performance, verify the design, and confirm the ISO profile is correct.

| Script | What it does |
|--------|--------------|
| `run-vm.sh` | Boot the ISO in a QEMU virtual machine |
| `run_and_audit.sh` | Run the VM and capture audit results |
| `validate-profile.sh` | Verify the ISO profile is valid before building |
| `test_all_features.sh` | Exercise DIAD's features end-to-end |
| `verify_design_and_logos.py` | Check the UI design and logo assets |
| `comprehensive_stress_audit.py` | Stress-test the system |
| `stress_test_workspaces.py` | Stress-test the workspace layout |
| `qmp_screenshot.py` / `qmp_capture_audit.py` | Capture VM screenshots / audit via QMP |

These are developer tools, not part of the installed OS. See
[`../STRUCTURE.md`](../STRUCTURE.md) for the full picture.
