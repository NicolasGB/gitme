# gitme 🚀

**gitme** is a terminal user interface (TUI) application designed to help you efficiently manage your GitHub pull requests directly from your terminal. Stay on top of reviews assigned to you and track your own open pull requests across multiple repositories.

## ✨ Features

- **Review Requested:** See all pull requests where your review is requested.
- **My Pull Requests:** List all pull requests you have opened.
- **Search:** Filter both "Review Requested" and "My Pull Requests" lists by repository name, PR ID, or PR title.
- **PR Details:** View pull request details, including description, status, reviewers, and labels.
- **Actions:** Quickly open PRs in the browser or copy their URLs.
- **Live Updates:** Automatically refreshes PR lists to show the latest changes.
- **Keyboard Shortcuts:** Navigate and manage PRs efficiently with customizable keyboard shortcuts.

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

- [x] Search in Pr (Partially done, missing other fields of search)
- [x] Refetch periodically
- [x] Refetch on demand
- [x] Add better keybindings such as `(n/p)` for navigation through repositories
- [x] Manage config(s) through CLI commands (Partially done)
- [ ] Copy pr links directly with `y` binding
- [ ] Add review history in details view
- [ ] Open repositories and not just PRs
- [ ] Build multi-user and multi-config as a first class citizen
- [ ] Package and publish

## 🙏 Special Mentions

- Inspired by the fantastic [lazygit](https://github.com/jesseduffield/lazygit).
- Built with the amazing [ratatui](https://ratatui.rs/) TUI framework.
