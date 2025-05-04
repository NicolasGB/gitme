# gitme 🚀

**gitme** is a terminal user interface (TUI) application designed to help you efficiently manage your GitHub pull requests directly from your terminal. Stay on top of reviews assigned to you and track your own open pull requests across multiple repositories.

## ✨ Features

- **Dual PR Views:** Easily switch between viewing Pull Requests requesting your review and Pull Requests assigned to you.
- **Repository Grouping:** PRs are grouped by repository for better organization.
- **Expand/Collapse:** Expand or collapse repository groups to focus on what matters.
- **GitHub Integration:** Fetches real-time PR data using the GitHub API.
- **Quick Actions:**
  - Open PRs directly in your web browser.
  - Trigger a custom "review" command (e.g., open your terminal/editor in the project's local directory).
- **Configurable:** Define which repositories to watch and customize behavior via a simple TOML configuration file.

## 📦 Installation

Ensure you have Rust and Cargo installed on your system. You can install `gitme` using Cargo:

```bash
cargo install --git https://github.com/NicolasGB/gitme.git
```

```bash
cargo install gitme
```

## ⚙️ Configuration

`gitme` requires a configuration file to know which repositories to monitor and how to authenticate with GitHub.

1.  **Create the configuration directory:**
    ```bash
    mkdir -p ~/.config/gitme
    ```
2.  **Create the configuration file:** `~/.config/gitme/config.toml`
3.  **Populate the file:**

    ```toml
    # Your GitHub username. Used to filter PRs assigned to you or requesting your review.
    username = "your-github-username"

    # GitHub Personal Access Token (PAT) with 'repo' scope.
    # Create one here: https://github.com/settings/tokens
    # Keep this secret!
    api_key = "ghp_YourGitHubPersonalAccessToken"

    # Optional: Define a custom command to run when you trigger the 'review' action (default: $TERMINAL or 'ghostty').
    # command = "ghostty"

    # Optional: Arguments to pass to the custom command.
    # command_args = ["-e", "nvim -c \"Octo pr list\""] # Example for ghostty opening neovim and launching the `:Octo pr list` command.

    # List of repositories to monitor.
    [[repositories]]
    owner = "NicolasGB" # The GitHub organization or user owning the repository
    name = "gitme"      # The name of the repository
    # Optional: The absolute local path to the repository on your system.
    # Used for the 'review' action to open a terminal/editor in this directory.
    system_path = "/path/to/your/local/clone/of/gitme"

    [[repositories]]
    owner = "another-owner"
    name = "repo2-name"
    system_path = "/path/to/your/local/clone/of/repo2"

    # Add more [[repositories]] blocks as needed
    ```

**Important:**

- Replace placeholder values (`your-github-username`, `ghp_...`, paths, owner/names) with your actual information.
- The `api_key` needs a GitHub Personal Access Token (PAT) with at least the `repo` scope to read repository data, including pull requests.

## 🚀 Usage

Simply run the application from your terminal:

```bash
gitme
```

### Keybindings

- **`↑` / `k`**: Scroll Up
- **`↓` / `j`**: Scroll Down
- **`Tab`**: Switch between "Review Requested" and "My Pull Requests" panels.
- **`Enter`**: Toggle expand/collapse for the selected repository group.
- **`z`**: Expand all repository groups in the current panel.
- **`c`**: Collapse all repository groups in the current panel.
- **`o`**: Open the selected Pull Request in your default web browser.
- **`r`**: Trigger the "Review" action (runs the configured `command` in the `system_path` if set).
- **`?`**: Show/Hide the keybindings help popup.
- **`Esc`**: Close the keybindings help popup.
- **`q`**: Quit the application.

## 🛠️ Development

1.  Clone the repository:
    ```bash
    git clone git@github.com:NicolasGB/gitme.git
    cd gitme
    ```
2.  Build:
    ```bash
    cargo build
    ```
3.  Run:
    ```bash
    cargo run
    ```

## 📄 License

This project is licensed under the MIT License - see the [LICENSE.md](LICENSE) file for details.

# TODO

- [x] Search in Prs
- [x] Refetch periodically
- [x] Refetch on demand
- [ ] Add better keybindings such as `(d/u)` for navigation through repositories
- [ ] Open repositories and not just PRs
- [ ] Manage config(s) through CLI commands
- [ ] Build multi-user and multi-config as a first class citizen
- [ ] Package and publish

## 🙏 Special Mentions

- Inspired by the fantastic [lazygit](https://github.com/jesseduffield/lazygit).
- Built with the amazing [ratatui](https://ratatui.rs/) TUI framework.
