use kaspascript_codegen::{
    bytecode_asm, bytecode_hex, compile_file, verify_artifact, CodegenError,
};
use pretty_assertions::assert_eq;

const CONTRACTS: &[(&str, &str)] = &[
    (
        "escrow.ks",
        include_str!("../../../tests/contracts/escrow.ks"),
    ),
    (
        "timelock.ks",
        include_str!("../../../tests/contracts/timelock.ks"),
    ),
    (
        "multisig.ks",
        include_str!("../../../tests/contracts/multisig.ks"),
    ),
    (
        "atomic_swap.ks",
        include_str!("../../../tests/contracts/atomic_swap.ks"),
    ),
    (
        "vault.ks",
        include_str!("../../../tests/contracts/vault.ks"),
    ),
    (
        "dagsafe_channel.ks",
        include_str!("../../../tests/contracts/dagsafe_channel.ks"),
    ),
];

const GOLDENS: &[(&str, &str, &str, &str, &str)] = &[
    (
        "escrow.ks",
        include_str!("../../../tests/contracts/escrow.ks"),
        include_str!("../../../tests/golden/escrow.artifact.json"),
        include_str!("../../../tests/golden/escrow.expected.hex"),
        include_str!("../../../tests/golden/escrow.expected.asm"),
    ),
    (
        "timelock.ks",
        include_str!("../../../tests/contracts/timelock.ks"),
        include_str!("../../../tests/golden/timelock.artifact.json"),
        include_str!("../../../tests/golden/timelock.expected.hex"),
        include_str!("../../../tests/golden/timelock.expected.asm"),
    ),
    (
        "multisig.ks",
        include_str!("../../../tests/contracts/multisig.ks"),
        include_str!("../../../tests/golden/multisig.artifact.json"),
        include_str!("../../../tests/golden/multisig.expected.hex"),
        include_str!("../../../tests/golden/multisig.expected.asm"),
    ),
    (
        "atomic_swap.ks",
        include_str!("../../../tests/contracts/atomic_swap.ks"),
        include_str!("../../../tests/golden/atomic_swap.artifact.json"),
        include_str!("../../../tests/golden/atomic_swap.expected.hex"),
        include_str!("../../../tests/golden/atomic_swap.expected.asm"),
    ),
    (
        "vault.ks",
        include_str!("../../../tests/contracts/vault.ks"),
        include_str!("../../../tests/golden/vault.artifact.json"),
        include_str!("../../../tests/golden/vault.expected.hex"),
        include_str!("../../../tests/golden/vault.expected.asm"),
    ),
];

#[test]
fn compiles_all_v1_contract_patterns() {
    for (file, source) in CONTRACTS {
        let artifact = compile_file(source, file).expect(file);
        verify_artifact(&artifact).expect(file);
        assert!(!artifact.bytecode.is_empty(), "{file}");
        assert!(artifact.warnings.is_empty(), "{file}");
        assert_eq!(artifact.target, "verified-tn12");
        bytecode_asm(&artifact.bytecode).expect(file);
    }
}

#[test]
fn escrow_bytecode_is_deterministic_across_1000_compiles() {
    let source = include_str!("../../../tests/contracts/escrow.ks");
    let first = compile_file(source, "escrow.ks").expect("first compile");

    for _ in 0..1_000 {
        let next = compile_file(source, "escrow.ks").expect("repeat compile");
        assert_eq!(next.bytecode, first.bytecode);
        assert_eq!(next.source_hash, first.source_hash);
        assert_eq!(bytecode_hex(&next.bytecode), bytecode_hex(&first.bytecode));
    }
}

#[test]
fn golden_artifacts_match_source_to_bytecode_outputs() {
    for (file, source, artifact_json, expected_hex, expected_asm) in GOLDENS {
        let artifact = compile_file(source, file).expect(file);
        verify_artifact(&artifact).expect(file);

        let actual_json = serde_json::to_string_pretty(&artifact).expect("artifact json");
        assert_eq!(
            actual_json.trim_end(),
            artifact_json.trim_end(),
            "{file} json"
        );
        assert_eq!(
            bytecode_hex(&artifact.bytecode),
            expected_hex.trim(),
            "{file} hex"
        );
        assert_eq!(
            bytecode_asm(&artifact.bytecode).expect(file),
            expected_asm.trim(),
            "{file} asm"
        );
    }
}

#[test]
fn negative_contracts_fail_loudly() {
    let cases = [
        (
            "wrong_signature_type.ks",
            r#"
            contract BadSig {
              params { owner: PublicKey }
              spend s(sig: PublicKey) {
                require sig.verify(owner);
              }
            }
            "#,
            "verify` receiver must be Signature",
        ),
        (
            "invalid_input_index.ks",
            r#"
            contract BadIndex {
              params { owner: PublicKey }
              spend s(sig: Signature) {
                require sig.verify(owner);
                require input(-1).value >= output(0).value;
              }
            }
            "#,
            "input/output index must be a non-negative integer literal",
        ),
        (
            "bad_finality_depth.ks",
            r#"
            contract BadFinality {
              params { owner: PublicKey, finality_depth: 0 }
              spend s(sig: Signature) {
                require sig.verify(owner);
              }
            }
            "#,
            "`finality_depth` must be > 0",
        ),
    ];

    for (file, source, expected) in cases {
        let err = compile_file(source, file).expect_err(file);
        assert!(err.to_string().contains(expected), "{file}: {err}");
    }
}

#[test]
fn unsupported_covenant_feature_fails_compilation() {
    let source = r#"
        contract FutureVault {
          params { owner: PublicKey, finality_depth: 10 }
          spend s(sig: Signature) {
            require sig.verify(owner);
            require covenant_id.depth >= 1;
          }
        }
    "#;

    let err = compile_file(source, "future_vault.ks").expect_err("covenant unsupported");
    assert!(matches!(
        err,
        CodegenError::GatedGrounding { ref id, .. } if id == "kip-20"
    ));
}
