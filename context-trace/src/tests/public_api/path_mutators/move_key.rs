//! Tests for MoveKey trait and AtomPosition operations
//!
//! The MoveKey trait enables moving positions forward (Right) or backward (Left)
//! by a given delta. This is used extensively in search to track atom positions.

#[cfg(test)]
use crate::*;

#[test]
fn atom_position_basic_creation() {
    let _tracing = init_test_tracing!();

    let pos = AtomPosition::from(5);
    assert_eq!(*pos, 5);
    assert_eq!(Into::<usize>::into(pos), 5);
}

#[test]
fn atom_position_add_operations() {
    let _tracing = init_test_tracing!();

    let mut pos = AtomPosition::from(10);

    // Test Add
    let new_pos = pos + 5;
    assert_eq!(*new_pos, 15);
    assert_eq!(*pos, 10); // Original unchanged

    // Test AddAssign
    pos += 3;
    assert_eq!(*pos, 13);
}

#[test]
fn atom_position_sub_operations() {
    let _tracing = init_test_tracing!();

    let mut pos = AtomPosition::from(10);

    // Test Sub
    let new_pos = pos - 3;
    assert_eq!(*new_pos, 7);
    assert_eq!(*pos, 10); // Original unchanged

    // Test SubAssign
    pos -= 4;
    assert_eq!(*pos, 6);
}

#[test]
fn atom_position_move_key_right() {
    let _tracing = init_test_tracing!();

    use crate::{
        direction::Right,
        path::mutators::move_path::key::MoveKey,
    };

    let mut pos = AtomPosition::from(5);
    <AtomPosition as MoveKey<Right>>::move_key(&mut pos, 3);
    assert_eq!(*pos, 8);
}

#[test]
fn atom_position_move_key_left() {
    let _tracing = init_test_tracing!();

    use crate::{
        direction::Left,
        path::mutators::move_path::key::MoveKey,
    };

    let mut pos = AtomPosition::from(10);
    <AtomPosition as MoveKey<Left>>::move_key(&mut pos, 4);
    assert_eq!(*pos, 6);
}

#[test]
fn atom_position_advance_key() {
    let _tracing = init_test_tracing!();

    use crate::path::mutators::move_path::key::AdvanceKey;

    let mut pos = AtomPosition::from(0);
    pos.advance_key(7);
    assert_eq!(*pos, 7);

    pos.advance_key(3);
    assert_eq!(*pos, 10);
}

#[test]
fn atom_position_retract_key() {
    let _tracing = init_test_tracing!();

    use crate::path::mutators::move_path::key::RetractKey;

    let mut pos = AtomPosition::from(20);
    pos.retract_key(5);
    assert_eq!(*pos, 15);

    pos.retract_key(10);
    assert_eq!(*pos, 5);
}

#[test]
fn atom_position_zero_moves() {
    let _tracing = init_test_tracing!();

    use crate::{
        direction::{
            Left,
            Right,
        },
        path::mutators::move_path::key::{
            AdvanceKey,
            MoveKey,
            RetractKey,
        },
    };

    let mut pos1 = AtomPosition::from(10);
    <AtomPosition as MoveKey<Right>>::move_key(&mut pos1, 0);
    assert_eq!(*pos1, 10);

    let mut pos2 = AtomPosition::from(10);
    <AtomPosition as MoveKey<Left>>::move_key(&mut pos2, 0);
    assert_eq!(*pos2, 10);

    let mut pos3 = AtomPosition::from(10);
    pos3.advance_key(0);
    assert_eq!(*pos3, 10);

    let mut pos4 = AtomPosition::from(10);
    pos4.retract_key(0);
    assert_eq!(*pos4, 10);
}

#[test]
fn atom_position_chain_operations() {
    let _tracing = init_test_tracing!();

    let mut pos = AtomPosition::from(0);
    pos += 5;
    pos += 3;
    pos -= 2;
    assert_eq!(*pos, 6);

    pos = pos + 10 - 4;
    assert_eq!(*pos, 12);
}
