# Verified tenant delivery

Run only the declared, locked local Perl script on the reviewed synthetic
orders.csv and shipments.csv. Retain Verify outputs with --delivery-dir and
independently check them with the included SkillSync composite-key contract.
No model, network, database, credential or payment operation is requested.

The script scopes local order IDs by tenant, groups split shipments and preserves
expectations separately. Its successful execution does not approve allocation
correctness: a balanced wrong allocation also produces a successful delivery
that the independent consumer must reject.
