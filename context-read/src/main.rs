mod context_read;

fn main() {
    #[cfg(test)]
    context_read::tests::grammar::test_grammar()
}
