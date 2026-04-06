#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

if [[ ! -f Cargo.toml ]]; then
  echo "Error: Cargo.toml not found. Run this script from the WhatSpace project." >&2
  exit 1
fi

mkdir -p bundles/alice bundles/bob bundles/carol
printf '{\n  "bundles": []\n}\n' > bundles/alice/bundles.json
printf '{\n  "bundles": []\n}\n' > bundles/bob/bundles.json
printf '{\n  "bundles": []\n}\n' > bundles/carol/bundles.json

TEST_OUTPUT="/tmp/whitespace_ack_test_output.log"
{
  echo 'peers alice add bob'
  echo 'peers bob add alice'
    echo 'peers bob add carol'
    echo 'peers carol add bob'

  echo 'start alice --server 127.0.0.1:8080'
  echo 'start bob --server 127.0.0.1:8080'
    echo 'start carol --server 127.0.0.1:8080'

    # Multi-hop send: alice -> bob -> carol
    echo 'send --from alice --to carol --message a2c_ack_check --ttl 180'
    sleep 2

    # Reverse-path send: carol -> bob -> alice
    echo 'send --from carol --to alice --message c2a_ack_check --ttl 180'
    sleep 2

  echo 'status alice'
  echo 'status bob'
    echo 'status carol'
  echo 'quit'
} | cargo run --quiet > "$TEST_OUTPUT" 2>&1

echo "=== CLI output (tail) ==="
tail -n 40 "$TEST_OUTPUT"

# python3 - <<'PY'
# import json
# import pathlib
# import sys

# root = pathlib.Path(".")
# alice_path = root / "bundles" / "alice" / "bundles.json"
# bob_path = root / "bundles" / "bob" / "bundles.json"
# carol_path = root / "bundles" / "carol" / "bundles.json"

# alice = json.loads(alice_path.read_text())
# bob = json.loads(bob_path.read_text())
# carol = json.loads(carol_path.read_text())

# alice_bundles = alice.get("bundles", [])
# bob_bundles = bob.get("bundles", [])
# carol_bundles = carol.get("bundles", [])

# def delivered_data_with_msg(bundles, message):
#     for b in bundles:
#         kind = b.get("kind", {})
#         data = kind.get("Data")
#         if data and data.get("msg") == message and b.get("shipment_status") == "Delivered":
#             return b
#     return None

# def find_data_bundle_id_by_msg(bundles, message):
#     for b in bundles:
#         data = b.get("kind", {}).get("Data")
#         if data and data.get("msg") == message:
#             return b.get("id")
#     return None

# def has_ack_for_bundle(bundles, bundle_id):
#     for b in bundles:
#         ack = b.get("kind", {}).get("Ack")
#         if ack and ack.get("ack_bundle_id") == bundle_id:
#             return True
#     return False

# def has_source_data_left(bundles, message):
#     for b in bundles:
#         kind = b.get("kind", {})
#         data = kind.get("Data")
#         if data and data.get("msg") == message:
#             return True
#     return False

# # Under global delete-on-ACK policy, delivered DATA may be deleted from all nodes.
# # Use ACK references + source-data deletion as primary checks.
# delivered_at_carol = delivered_data_with_msg(carol_bundles, "a2c_ack_check")
# delivered_at_alice = delivered_data_with_msg(alice_bundles, "c2a_ack_check")

# a2c_id = (
#     find_data_bundle_id_by_msg(alice_bundles, "a2c_ack_check")
#     or find_data_bundle_id_by_msg(bob_bundles, "a2c_ack_check")
#     or find_data_bundle_id_by_msg(carol_bundles, "a2c_ack_check")
# )

# c2a_id = (
#     find_data_bundle_id_by_msg(carol_bundles, "c2a_ack_check")
#     or find_data_bundle_id_by_msg(bob_bundles, "c2a_ack_check")
#     or find_data_bundle_id_by_msg(alice_bundles, "c2a_ack_check")
# )

# bob_has_relay_artifacts = any(
#     ("Data" in b.get("kind", {})) or ("Ack" in b.get("kind", {}))
#     for b in bob_bundles
# )

# errors = []

# if a2c_id is None and delivered_at_carol is None:
#     errors.append("FAIL: Could not find data id for a2c_ack_check in any node.")
# if c2a_id is None and delivered_at_alice is None:
#     errors.append("FAIL: Could not find data id for c2a_ack_check in any node.")

# # If DATA still exists somewhere, use its id. Otherwise fallback to delivered record id if present.
# if a2c_id is None and delivered_at_carol is not None:
#     a2c_id = delivered_at_carol.get("id")
# if c2a_id is None and delivered_at_alice is not None:
#     c2a_id = delivered_at_alice.get("id")

# if a2c_id is not None:
#     ack_seen_for_a2c = (
#         has_ack_for_bundle(alice_bundles, a2c_id)
#         or has_ack_for_bundle(bob_bundles, a2c_id)
#         or has_ack_for_bundle(carol_bundles, a2c_id)
#     )
#     if not ack_seen_for_a2c:
#         errors.append("FAIL: No ACK found for a2c_ack_check.")

# if c2a_id is not None:
#     ack_seen_for_c2a = (
#         has_ack_for_bundle(alice_bundles, c2a_id)
#         or has_ack_for_bundle(bob_bundles, c2a_id)
#         or has_ack_for_bundle(carol_bundles, c2a_id)
#     )
#     if not ack_seen_for_c2a:
#         errors.append("FAIL: No ACK found for c2a_ack_check.")

# if has_source_data_left(alice_bundles, "a2c_ack_check"):
#     errors.append("FAIL: Alice still has source data a2c_ack_check after ACK (expected delete).")
# if has_source_data_left(carol_bundles, "c2a_ack_check"):
#     errors.append("FAIL: Carol still has source data c2a_ack_check after ACK (expected delete).")

# # Global-delete policy: once ACK propagates, original DATA should eventually disappear everywhere.
# if any(
#     has_source_data_left(node_bundles, "a2c_ack_check")
#     for node_bundles in (alice_bundles, bob_bundles, carol_bundles)
# ):
#     errors.append("FAIL: a2c_ack_check DATA still exists on at least one node under global delete policy.")

# if any(
#     has_source_data_left(node_bundles, "c2a_ack_check")
#     for node_bundles in (alice_bundles, bob_bundles, carol_bundles)
# ):
#     errors.append("FAIL: c2a_ack_check DATA still exists on at least one node under global delete policy.")

# if not bob_has_relay_artifacts:
#     errors.append("FAIL: Bob has no relay artifacts (Data/Ack), multi-hop path likely not exercised.")

# if errors:
#     print("\n".join(errors))
#     sys.exit(1)

# print("PASS: 3-node ACK checks succeeded under global delete-on-ACK policy.")
# PY
