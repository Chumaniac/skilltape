"""Exercise the publication shell against owned fake Git/GitHub executables."""
import os
from pathlib import Path
import subprocess
import tempfile
import unittest
from test_release_workflow import WORKFLOW, job_block, job_steps, named_step, run_script


class PublicationTests(unittest.TestCase):
    def test_an_existing_release_is_never_uploaded_over(self):
        script = run_script(named_step(job_steps(job_block(WORKFLOW.read_text(), 'publish')), 'Publish GitHub release'))
        script = script.replace('${{ needs.prepare.outputs.tag }}', 'v0.5.0')
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            (root / 'git').write_text('#!/bin/sh\nif [ "$1" = "ls-remote" ]; then printf "%s\\trefs/tags/v0.5.0\\n" "$GITHUB_SHA"; fi\n')
            (root / 'gh').write_text('#!/bin/sh\ncase "$2" in view) exit 0;; upload|create) echo "$2" >> "$PUBLICATION_LOG";; esac\n')
            for name in ['git', 'gh']:
                (root / name).chmod(0o700)
            log = root / 'mutations'
            result = subprocess.run(['bash', '-e', '-c', script], cwd=root, capture_output=True, env={**os.environ, 'PATH': str(root) + ':/usr/bin:/bin', 'GITHUB_SHA': 'a' * 40, 'GITHUB_REPOSITORY': 'synthetic/repo', 'PUBLICATION_LOG': str(log)})
            self.assertNotEqual(result.returncode, 0, 'an existing immutable release must stop publication')
            self.assertFalse(log.exists(), 'no create/upload is allowed for an existing release')


if __name__ == '__main__':
    unittest.main()
