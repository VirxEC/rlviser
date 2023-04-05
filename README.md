## RocketSim Visualizer

A light-weight visualizer for [rocketsim-rs](https://github.com/VirxEC/rocketsim-rs) binds that listens for UDP packets.

Any language can communicate with the visualizer by sending UDP packets in the correct format, but rocketsim-rs has a `GameState.to_bytes()` function that does this automatically.

### Usage

To see an example of how to communicate with the visualizer, see the [example](https://github.com/VirxEC/rocketsim-rs/blob/master/examples/rlviser_socket.rs) in the [rocketsim-rs](https://github.com/VirxEC/rocketsim-rs) repository.
