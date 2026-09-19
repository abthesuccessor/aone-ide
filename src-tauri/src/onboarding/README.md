# Permission-first project onboarding

This module owns the native authority boundary for creating a project directly
under the current macOS user's Documents directory, cloning a strictly bounded
public GitHub HTTPS repository, and inspecting sanitized Git onboarding
metadata after explicit native consent.

The renderer never supplies an absolute destination. After an explicit user
action, Rust resolves and attests Documents so the exact direct child can appear
in native consent; the proposed child is not probed and no mutation or network
activity occurs before approval. Clone and Create serialize with workspace
switching, revalidate directory identity, refuse existing targets, and open the
new folder through the same backend-only scanner/index installer used by the
native folder picker. Clone uses a credential-free, shallow, single-branch
ordinary tracked snapshot with submodules and Git LFS downloads disabled.

Git inspection is read-only. It parses bounded regular configuration files and
at most 16 non-symlink `.pub` files. Directory entries without the strict
public-key suffix are discarded before any metadata or content access, so
private key files are never opened or statted. Unquoted inline `#` and `;` Git
configuration comments are stripped before identities or remotes are reported;
remote credentials, query values, fragments, local paths, and unsupported forms
remain sanitized. The module never runs SSH, GitHub CLI, fork, branch, push, or
configuration mutations.
