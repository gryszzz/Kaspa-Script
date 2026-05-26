use kaspascript_codegen::{compile_file, verify_artifact};

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
];

#[test]
fn compiles_all_v1_contract_patterns() {
    for (file, source) in CONTRACTS {
        let artifact = compile_file(source, file).expect(file);
        verify_artifact(&artifact).expect(file);
        assert!(!artifact.bytecode.is_empty(), "{file}");
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
    }
}
