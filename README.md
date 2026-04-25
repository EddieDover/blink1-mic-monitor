# Mic Mute Status Monitor

Monitors microphone mute status from the selected audio input and reflects that state to a Blink(1) RGB device or the console.

## Install

Create a virtual environment and install the project into it:

```bash
uv sync
```

That installs the `micmute-status-monitor` command into `.venv/bin/`.

## Usage

List input devices:

```bash
uv run micmute-status-monitor --list-devices
```

Run with the Blink(1) output:

```bash
uv run micmute-status-monitor -o blink1
```

Use a specific microphone device:

```bash
uv run micmute-status-monitor -d 5 -o blink1
```

## User Service

A ready-to-copy user `systemd` unit is provided at `systemd/micmute-status-monitor.service`.

To install it for the current user:

```bash
mkdir -p ~/.config/systemd/user
cp systemd/micmute-status-monitor.service ~/.config/systemd/user/
systemctl --user daemon-reload
systemctl --user enable --now micmute-status-monitor.service
```

If you need a specific input device, edit the `ExecStart` line and add `-d N`.