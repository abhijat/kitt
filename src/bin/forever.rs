use std::hint::black_box;

fn forever(mut i: i32) -> ! {
    loop {
        i = 42;
    }
}
fn main() {
    let i = 0;
    black_box(forever(black_box(i)));
}
