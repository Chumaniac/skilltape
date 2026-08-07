# SkillTape Security

SkillTape treats workflows as untrusted input and execution as a high-risk
boundary. Its security goal is to constrain file, process, network, and
environment access while recording and replaying Agent Skills locally, and to
keep Tapes, Receipts, logs, and exports free of plaintext secrets.

## Threat model and boundary

SkillTape protects:

- Files and directories outside the workspace;
- Executable programs, network hosts, and environment variables not declared in
  `permissions`;
- Command injection, path traversal, symlink escapes, and leftover background
  processes;
- Plaintext secrets in Receipts, Capture output, and environment snapshots.

Replay/Verify runs through a platform-restricted executor: Linux uses
`bubblewrap`, and macOS uses `/usr/bin/sandbox-exec`. The CLI also rejects
dangerous commands, paths outside the workspace, disabled network access, and
secret environment identifiers at the policy layer. The restricted executor
is part of defense in depth; it must not be understood as permission to run
arbitrary unreviewed code.

The following are not substitutes for the security boundary:

- Permissions actively granted by a user may allow a Skill to read or modify
  data within that permission scope;
- Vulnerabilities in a host, kernel, operating system, or third-party binary
  already trusted by the user are not fixed by SkillTape alone;
- Unsupported operating systems are not simulated as secure isolation
  environments, and Replay/Verify must fail explicitly.

## Secret handling

Capture does not read environment-variable values by default. It records only
the names, lengths, and SHA-256 metadata of variables in the explicit allowlist.
Terminal output is redacted before persistence for named secrets, common tokens,
and configuration patterns. Receipts and Replay summaries retain only output
summaries, lengths, and policy decisions; they do not retain raw stdout/stderr.

Tapes, Receipts, export directories, temporary logs, and CI workspaces may still
contain paths, command names, file sizes, and other sensitive metadata. Do not
upload unreviewed artifacts to a public repository or CI artifact. Do not write
real credentials into tests, Issues, commit messages, or logs.

## Platform compatibility

| Platform | Capture / Compile / Lint / Export | Replay / Verify | Restricted executor and differences |
| --- | --- | --- | --- |
| macOS | Supported | Requires `/usr/bin/sandbox-exec` | PTY uses the system implementation; file watching uses the platform watcher; the sandbox profile opens only the temporary workspace |
| Linux | Supported | Requires `bwrap`/`bubblewrap` and an available user namespace | `bwrap --unshare-all` isolates network, environment, and filesystem; CI preinstalls bubblewrap |
| Windows | Supports non-execution commands and package operations | Fails closed and returns sandbox unavailable | Release packages may use the Windows installer; Replay/Verify support is not claimed until an equivalent restricted executor is integrated |

The complete release gates run only on the Linux and macOS matrix. PTY terminal
size, signal semantics, and filesystem-watcher event coalescing may differ by
platform; Tape and Receipt schemas must not depend on the exact time values of
these nondeterministic fields.

## Vulnerability disclosure

Do not publish exploitable sandbox escapes, path traversal, secret leakage, or
remote-execution details in a public Issue. Prefer creating a Private
Vulnerability Report through the repository's GitHub Security page. If that
entry point is unavailable, first send the maintainers a private message
without exploit details and wait for a security response before disclosure.

A report should include the affected version and platform, minimal
reproduction steps, expected and actual behavior, whether specific
`permissions` are required, whether files outside the workspace can be read or
secrets leaked, and logs or patches that contain no credentials. First remove
real tokens, cookies, private keys, and production data.

After the impact is confirmed, security fixes are released through protected
commits and release notes. Whether a fix is backported depends on whether the
affected version remains within its support window.

## Version and compatibility policy

- The CLI and public schemas follow SemVer; the current development line is
  `0.x`, so minor versions may still adjust experimental CLI behavior.
- The Tape, Receipt, Run, and Plugin Export protocols use explicit
  `skilltape.dev/.../v1` schema identifiers. An incompatible change must create
  a new version identifier and preserve the old read path, or state migration
  requirements explicitly in the release notes.
- Permission defaults, sandbox configuration, and secret-redaction rules are
  security behavior. Security tightening may ship in a patch release; relaxing
  default permissions requires independent review and a recorded migration
  impact.
- GitHub Actions runs only local code and reviewed fixtures; it does not upload
  Tapes, Receipts, logs, or environment snapshots.

## Local security gates

```bash
cargo test -p skilltape-cli --test security_path_escape --test security_secret_leak --test integration_full_journey -- --test-threads=1
cargo clippy --workspace --all-targets -- -D warnings
cargo bench -p skilltape-cli --bench capture_compile
SKILLTAPE_BENCHMARK_LARGE=1 cargo bench -p skilltape-cli --bench capture_compile
```

The benchmark commands do not define an uncalibrated performance pass line;
they are used to observe regression trends. The large-log scenario uses a
sparse file and reads it sequentially as needed, and should still run on a
dedicated runner.
