//! String manipulation utilities

///// Field visitor to extract a specific field value
//pub(super) struct FieldExtractor {
//    pub(super) field_name: &'static str,
//    pub(super) value: Option<String>,
//}
//
//impl FieldExtractor {
//    pub(super) fn new(field_name: &'static str) -> Self {
//        Self {
//            field_name,
//            value: None,
//        }
//    }
//}

//impl Visit for FieldExtractor {
//    fn record_debug(
//        &mut self,
//        field: &Field,
//        value: &dyn std::fmt::Debug,
//    ) {
//        if field.name() == self.field_name {
//            self.value = Some(format!("{:?}", value));
//        }
//    }
//
//    fn record_str(
//        &mut self,
//        field: &Field,
//        value: &str,
//    ) {
//        if field.name() == self.field_name {
//            self.value = Some(value.to_string());
//        }
//    }
//}

/// Strip ANSI escape codes from a string
pub(super) fn strip_ansi_codes(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut chars = s.chars();

    while let Some(ch) = chars.next() {
        if ch == '\x1b' {
            // Skip escape sequence
            if let Some('[') = chars.next() {
                // Skip until we find a letter (the command character)
                for ch in chars.by_ref() {
                    if ch.is_ascii_alphabetic() {
                        break;
                    }
                }
            }
        } else {
            result.push(ch);
        }
    }

    result
}
