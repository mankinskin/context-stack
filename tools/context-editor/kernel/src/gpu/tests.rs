#[test]
fn svo_double_buffer_swap_logic() {
    // Verify the swap pointer logic without a GPU device.
    // We can't call SvoDoubleBuffer::new without a RenderDevice, so we
    // test the invariants with a hand-rolled struct.
    struct FakeSvo {
        current_is_front: bool,
    }
    impl FakeSvo {
        fn write_is_back(&self) -> bool {
            self.current_is_front
        }
        fn read_is_front(&self) -> bool {
            self.current_is_front
        }
        fn swap(&mut self) {
            self.current_is_front = !self.current_is_front;
        }
    }

    let mut svo = FakeSvo {
        current_is_front: true,
    };
    assert!(svo.read_is_front(), "before swap: GPU reads front");
    assert!(svo.write_is_back(), "before swap: WASM writes back");
    svo.swap();
    assert!(
        !svo.read_is_front(),
        "after swap: GPU reads back (old back)"
    );
    assert!(
        !svo.write_is_back(),
        "after swap: WASM writes front (old front)"
    );
    svo.swap();
    assert!(svo.read_is_front(), "double swap: back to original");
}
