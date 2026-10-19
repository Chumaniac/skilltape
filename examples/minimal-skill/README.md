# minimal-skill

A valid minimal SkillTape package runs a locked local Perl script that creates
`fixtures/output/message.txt` containing `hello from SkillTape\n`. Declaring an
output does not redirect stdout: the script explicitly writes the file.

On macOS/Linux, verify the reviewed reference bytes:

```sh
skilltape lint examples/minimal-skill --strict --json
skilltape verify examples/minimal-skill --expect-file fixtures/output/message.txt \
  --expect-sha256 fixtures/output/message.txt=175b4d68597accf8585303fd6b9e7ab588a23a0f9a2d9cdb1647c3130ed63a67 --json
```

See the [reference index](../../docs/reference/README.md) for package schema
contracts and the [guides index](../../docs/guides/README.md) for installation
and operational guidance. Replay/Verify continue to fail closed on Windows.
