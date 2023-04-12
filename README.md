## RocketSim Visualizer

A light-weight visualizer for [rocketsim-rs](https://github.com/VirxEC/rocketsim-rs) binds that listens for UDP packets.

Any language can communicate with the visualizer by sending UDP packets in the correct format, but rocketsim-rs has a `GameState.to_bytes()` function that does this automatically.

![image](https://user-images.githubusercontent.com/35614515/231363188-0b9baee9-e39a-4060-8d54-7e4dbab2ff80.png)

### Running

You must have [umodel](https://www.gildor.org/en/projects/umodel) in your root directory along with an `assets.path` file that points to your `rocketleague/TAGame/CookedPCConsole` directory so the visualizer can uncook the game assets.

Precompiled versions of umodel for Windows and Linux are available on the website linked above.

### Usage

To see an example of how to communicate with the visualizer, see the [example](https://github.com/VirxEC/rocketsim-rs/blob/master/examples/rlviser_socket.rs) in the [rocketsim-rs](https://github.com/VirxEC/rocketsim-rs) repository.
