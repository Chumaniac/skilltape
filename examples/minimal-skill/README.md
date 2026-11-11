# minimal-skill

A minimal SkillTape package copies the reviewed `inputs/sample.txt` into the
declared `message.txt` output using the system `/bin/cp`. Declaring an output
does not redirect stdout: the workflow explicitly writes a file. It needs no
interpreter or additional runtime dependency on macOS/Linux.

Verify the original fixture bytes against an independent reference digest:

```sh
skilltape lint examples/minimal-skill --strict --json
skilltape verify examples/minimal-skill --input examples/minimal-skill/fixtures/input \
  --expect-file message.txt \
  --expect-sha256 message.txt=175b4d68597accf8585303fd6b9e7ab588a23a0f9a2d9cdb1647c3130ed63a67 --json
```

The fixture contains `hello from SkillTape\n`. See the [reference index](../../docs/reference/README.md)
and [guides index](../../docs/guides/README.md). Replay/Verify continue to fail
closed on Windows.
