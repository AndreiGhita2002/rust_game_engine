use rust_game_engine::run;

fn main() {
    // Don't use block_on inside of an async function
    // if you plan to support WASM. Futures have to be run
    // using the browser's executor. If you try to bring your
    // own your code will crash when you encounter a future
    // that doesn't execute immediately.
    pollster::block_on(run());
}
