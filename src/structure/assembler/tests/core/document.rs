use super::*;

#[test]
fn empty_document_produces_document_root() {
    let asm = Assembler::with_defaults();
    let result = asm.finish();

    assert_eq!(result.root.kind, NodeKind::Document);
    assert!(result.root.children.is_empty());
    assert!(result.warnings.is_empty());
}
