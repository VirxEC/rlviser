## RocketSim Visualizer

[![forthebadge](https://forthebadge.com/images/badges/made-with-rust.svg)](https://forthebadge.com)

A lightweight visualizer for [rocketsim-rs](https://github.com/VirxEC/rocketsim-rs) binds that listens for UDP packets.

Any language can communicate with the visualizer by sending UDP packets in the correct format, but `rocketsim-rs` has a `GameState.to_bytes()` function that does this automatically.

![image](https://github.com/VirxEC/rlviser/assets/35614515/47613661-754a-4549-bcef-13df399645be)

### First-time Launch Setup

On launch rlviser looks for `ulmodel.exe` or `ulmodel` binaries in the same directory, it will pull the assets needed for you.

You should have [UModel](https://www.gildor.org/en/projects/umodel) so the visualizer can uncook the game assets into the `assets/` directory.

Precompiled versions of UModel for Windows and Linux are available on the website linked above, but Linux users may have to compile it themself.

If you don't include UModel, the visualizer will use some minimalist default assets.
Certain things may not be present due to the lack of assets, but the field, cars, and ball are visible.

### Usage

To see an example of how to communicate with the visualizer, see the [example](https://github.com/VirxEC/rocketsim-rs/blob/master/examples/rlviser_socket.rs) in the [rocketsim-rs](https://github.com/VirxEC/rocketsim-rs) repository.

You can also choose to use the integrated support in [RLGym 2.0](https://github.com/lucas-emery/rocket-league-gym) and [RLGym-PPO](https://github.com/AechPro/rlgym-ppo) or use the [RLViser-Py](https://pypi.org/project/rlviser-py/) library to interface directly from Python via [RocketSim](https://pypi.org/project/RocketSim/) classes.

### Controls

**NOTICE:** These controls WON'T WORK until you've toggled the menu off. The menu is open by default upon launch.

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
| `Left Ctrl` | Move down |
| `Left Shift` | Slow |
| `R` | State set ball towards goal |
| `P` | Toggle pause/play |
| `+` | Increase game speed +0.5x |
| `-` | Decrease game speed -0.5x |
| `=` | Set game speed to 1x |
| `Left click`<sup>1</sup> | Drag cars and ball |

<sup>1</sup> - Requires the menu toggled ON to free the cursor, you can drag cars and the ball to move them in the world. Requires the agent on the other side to support state setting.

## Modes

Currently, both standard soccer and hoops are supported.

![image](https://github.com/VirxEC/rlviser/assets/35614515/d804d7e5-b78e-4a0a-9133-38e5aed0681d)
