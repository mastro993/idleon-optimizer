//! The jar tables the save rows index into.

use idleon_jar as jar;

#[test]
fn the_jar_tables_line_up_with_the_save_rows() {
    assert_eq!(jar::TYPE_NAMES.len(), jar::TYPE_CATEGORIES.len());
    assert_eq!(jar::RUPIE_NAMES.len(), jar::RUPIE_COLORS.len());
    assert_eq!(jar::TYPE_CATEGORIES[7], jar::TypeCategory::Gilded);
    assert_eq!(jar::TYPE_CATEGORIES[1].label(), "chance");
}
