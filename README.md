## RocketSim Visualizer

[![forthebadge](https://forthebadge.com/images/badges/made-with-rust.svg)](https://forthebadge.com)

A light-weight visualizer for [rocketsim-rs](https://github.com/VirxEC/rocketsim-rs) binds that listens for UDP packets.

Any language can communicate with the visualizer by sending UDP packets in the correct format, but rocketsim-rs has a `GameState.to_bytes()` function that does this automatically.

![image](https://raw.githubusercontent.com/VirxEC/rlviser/master/rlviser.png)

### First-time Launch Setup

You must have [umodel](https://www.gildor.org/en/projects/umodel) in your root directory along with an `assets.path` file that points to your `rocketleague/TAGame/CookedPCConsole` directory so the visualizer can uncook the game assets into the `assets/` directory.

Precompiled versions of umodel for Windows and Linux are available on the website linked above.

### Usage

To see an example of how to communicate with the visualizer, see the [example](https://github.com/VirxEC/rocketsim-rs/blob/master/examples/rlviser_socket.rs) in the [rocketsim-rs](https://github.com/VirxEC/rocketsim-rs) repository.

### Controls

| Key | Action |
| --- | --- |
| `Esc` | Toggle menu |
| `1` - `6` | Change car camera focus |
| `9` | Director camera |
| `0` | Free camera |
| `W` | Move forward |
| `A` | Move left |
| `S` | Move backward |
| `D` | Move right |
| `Space` | Move up |
| `Left Shift` | Move down |
| `R` | State set ball towards goal |

With the menu toggled on, you can drag cars and the ball with your cursor to move them in the world.
