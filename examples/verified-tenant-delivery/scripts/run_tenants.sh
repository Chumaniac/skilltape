#!/bin/bash
set -eu

# Apple ships a version-selecting /usr/bin/perl launcher. Use a declared
# native interpreter when available, without changing the sandbox or env.
for interpreter in /usr/bin/perl5.34 /usr/bin/perl5.30 /usr/bin/perl; do
    if [ -x "$interpreter" ]; then
        exec "$interpreter" scripts/produce_tenants.pl "$@"
    fi
done
printf '%s\n' 'tenant-delivery interpreter is unavailable' >&2
exit 1
