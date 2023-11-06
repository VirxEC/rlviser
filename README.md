## RocketSim Visualizer

[![forthebadge](https://forthebadge.com/images/badges/made-with-rust.svg)](https://forthebadge.com)

A light-weight visualizer for [rocketsim-rs](https://github.com/VirxEC/rocketsim-rs) binds that listens for UDP packets.

Any language can communicate with the visualizer by sending UDP packets in the correct format, but rocketsim-rs has a `GameState.to_bytes()` function that does this automatically.

![rlviser standard](https://github.com/VirxEC/rlviser/assets/35614515/5dbae568-2ecb-4c5d-a645-81c8f171f146)

### First-time Launch Setup

You must have [umodel](https://www.gildor.org/en/projects/umodel) in your root directory along with an `assets.path` file that points to your `rocketleague/TAGame/CookedPCConsole` directory so the visualizer can uncook the game assets into the `assets/` directory.

Precompiled versions of umodel for Windows and Linux are available on the website linked above.

### Usage

To see an example of how to communicate with the visualizer, see the [example](https://github.com/VirxEC/rocketsim-rs/blob/master/examples/rlviser_socket.rs) in the [rocketsim-rs](https://github.com/VirxEC/rocketsim-rs) repository.

You can also choose to use the integrated support in RLGym 2.0 or to use the [RLViser-Py](https://pypi.org/project/rlviser-py/) library to interface directly from Python via RocketSim.

### Controls

**NOTICE:** These controls WON'T WORK until you've toggle the menu off. The menu is open by default upon launch.

| Key | Action |
| --- | --- |
| `Esc` | Toggle menu |
| `1` - `8` | Change car camera focus |
| `9` | Director camera |
| `0` | Free camera |
| `W` | Move forward |
| `A` | Move left |
| `S` | Move backward |
| `D` | Move right |
| `Space` | Move up |
| `Left Shift` | Move down |
| `R` | State set ball towards goal |
| `Left click`<sup>1</sup> | Drag cars and ball |

 <sup>1</sup> - Requires the menu toggle ON to free the cursor, you can drag cars and the ball to move them in the world. Requires the agent on the other side to support state setting.

## Modes

Currently, both standard soccer and hoops are supported.

![rlviser hoops](https://github.com/VirxEC/rlviser/assets/35614515/20086dfa-e4c9-47c3-8900-91b172371e0a)
