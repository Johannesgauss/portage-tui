# Configuration & Safety Model

`portage-tui` is engineered specifically to avoid the primary failure mode of third-party package tools: **corrupting hand-crafted `/etc/portage/` configurations**.

---

## The Non-Destructive Storage Pattern

In Gentoo Linux, subdirectories under `/etc/portage/` (such as `package.use/`, `package.mask/`, `package.unmask/`, and `package.accept_keywords/`) are evaluated in **lexicographical (alphabetical) order**. Files that sort later override earlier files.

`portage-tui` writes all user-toggled settings strictly to:

```
/etc/portage/
├── package.use/
│   ├── 00-custom                ← Your manual config (NEVER TOUCHED)
│   └── zz-portage-manager       ← Managed by portage-tui
├── package.mask/
│   └── zz-portage-manager
├── package.unmask/
│   └── zz-portage-manager
└── package.accept_keywords/
    └── zz-portage-manager
```

### Key Guarantees:
1. **Zero Comment Loss**: Your comments, formatting, and custom files inside `/etc/portage/` are never overwritten, edited, or parsed destructively.
2. **Alphabetical Precedence**: Because `zz-*` sorts last, choices made inside `portage-tui` cleanly override previous defaults without touching the original files.
3. **Instant Factory Reset**: To completely remove all changes made by `portage-tui` and return your system to its original state, simply delete the `zz-portage-manager` files:
   ```bash
   sudo rm -f /etc/portage/*/zz-portage-manager
   ```

---

## Permission Architecture (Running as Non-Root)

You should **never** run graphical or interactive TUIs as `root`. To allow regular users to manage USE flags and package keywords safely:

```bash
# Run the setup script once:
sudo ./tools/setup-portage-permissions.sh $USER
```

### What this accomplishes:
1. **Adds user to the built-in `portage` group**:
   ```bash
   gpasswd -a $USER portage
   ```
2. **Assigns group ownership of `/etc/portage` to `portage`**:
   ```bash
   chgrp -R portage /etc/portage
   ```
3. **Grants group write permissions**:
   ```bash
   chmod -R g+w /etc/portage
   ```
4. **Applies directory setgid bit (`g+s`)**:
   ```bash
   find /etc/portage -type d -exec chmod g+s {} +
   ```
   The setgid bit ensures that any new files created inside `/etc/portage/` automatically inherit the `portage` group rather than the user's primary group.

### Activating Group Membership
After running the script, activate the new group token in your current terminal:
```bash
newgrp portage
```
*(Or log out and log back in).*
