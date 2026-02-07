
fn main() {
    #[cfg(feature = "test-api")]
    {
        context_read::tests::grammar::test_grammar()
    }
}
