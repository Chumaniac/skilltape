# Verified order summary delivery

Run only the declared local Perl script against the small synthetic order CSV.
Review the script and permissions first. Preserve a successful Verify with
--delivery-dir and independently check the actual CSV/JSON using the SkillSync
order-summary artifact contract. No model, network, database or credential is used.

The example computes daily counts and integer-cent totals; it is not a payment
processor, accounting decision, source-authentication system or production integration.
