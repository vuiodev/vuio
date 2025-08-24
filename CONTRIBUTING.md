
# Contributing to VuIO

First off, thank you for considering contributing to VuIO! We're thrilled you're interested in helping us build a better DLNA solution. Your contributions are invaluable to the project's success.

This document provides a set of guidelines for contributing to VuIO. These are mostly guidelines, not strict rules. Use your best judgment, and feel free to propose changes to this document in a pull request.

## Table of Contents

- [Code of Conduct](#code-of-conduct)
- [How Can I Contribute?](#how-can-i-contribute)
- [Reporting Bugs](#reporting-bugs)
- [Suggesting Enhancements](#suggesting-enhancements)
- [Submitting Code and Pull Requests](#submitting-code-and-pull-requests)
- [Your First Code Contribution](#your-first-code-contribution)
- [Development Setup](#development-setup)
- [Pull Request Process](#pull-request-process)
- [Contributor License Agreement (CLA)](#contributor-license-agreement-cla)
- [Coding Style](#coding-style)
- [Commit Message Guidelines](#commit-message-guidelines)
- [License](#license)

---

## Code of Conduct

This project and everyone participating in it is governed by our Code of Conduct. By participating, you are expected to uphold this code. Please report unacceptable behavior to [contact@vuio.dev].

---

## How Can I Contribute?

There are many ways to contribute, and many of them don't involve writing a single line of code.

### Reporting Bugs

Bugs are tracked as GitHub Issues. Before creating a bug report, please check the existing issues to see if someone has already reported it.

When you are creating a bug report, please include as many details as possible:

- A clear and descriptive title.
- A detailed description of the problem. Explain the behavior you saw and what you expected to see.
- Steps to reproduce the bug. Provide a minimal, step-by-step guide.
- Your environment. Include your operating system, VuIO version, and any other relevant software (e.g., media server, renderer).
- Logs or stack traces. If applicable, paste relevant logs within a code block.

### Suggesting Enhancements

We welcome suggestions for new features and improvements! Enhancements are also tracked as GitHub Issues.

- Use a clear and descriptive title to identify the suggestion.
- Provide a step-by-step description of the suggested enhancement in as much detail as possible.
- Explain why this enhancement would be useful to most VuIO users.
- Consider the DLNA/UPnP specifications. If your suggestion deviates from or extends the standard, please note this and provide a rationale.

---

## Submitting Code and Pull Requests

If you're ready to contribute code, that's fantastic! Please follow the process outlined below.

### Your First Code Contribution

Unsure where to begin? You can start by looking through `good first issue` and `help wanted` issues:

- **Good first issues** – issues which should only require a few lines of code and a test or two.
- **Help wanted issues** – issues which should be a bit more involved than good first issues.

### Development Setup

To get your local development environment set up, please follow these steps:

1. **Fork** the VuIO repository on GitHub.
2. **Clone** your fork locally:

	```sh
	git clone https://github.com/vuiodev/vuio.git
	```

3. **Install the project dependencies.**

	```sh
	# Add your project's build/setup commands here
	# e.g., for a Python project:
	pip install -r requirements-dev.txt
	```

4. **Create a new branch for your changes:**

	```sh
	git checkout -b my-awesome-feature
	```

5. **Set up the upstream remote to sync your fork with the main repository:**

	```sh
	git remote add upstream https://github.com/vuiodev/vuio.git
	```

---

## Pull Request Process

1. Make your changes in your local branch, adhering to the [Coding Style](#coding-style).
2. Add or update tests to cover your changes. We value well-tested code!
3. Ensure all tests pass:

	```sh
	# Add your project's test command here
	# e.g., pytest
	```

4. Update the documentation (`README.md`, `docs` folder, etc.) if your changes affect it.
5. Write clear, concise [Commit Messages](#commit-message-guidelines).
6. Push your branch to your GitHub fork:

	```sh
	git push origin my-awesome-feature
	```

7. Open a pull request to the `main` (or `develop`) branch of the VuIO repository.
8. Provide a clear title and description for your pull request, explaining the "what" and "why" of your changes. Link to any relevant issues.
9. Ensure you have signed the Contributor License Agreement (CLA). Pull requests cannot be merged until the CLA is signed.

---

## Contributor License Agreement (CLA)

Before we can accept your contribution, you need to sign our Contributor License Agreement (CLA).

### Why do we need a CLA?

The CLA is a standard practice in many large open-source projects. It serves two main purposes:

1. **It protects you, the contributor.** It clearly states that you are licensing your contributions to the project and are not giving away ownership of your original work beyond that license.
2. **It protects the project and its users.** It gives the project maintainers the necessary rights to manage the codebase, distribute it under the Apache 2.0 license, and defend it from legal challenges. This ensures that the project can remain open-source and freely available for everyone in the long term.

By signing the CLA, you grant VuIO a broad, perpetual, and irrevocable license to your contributions, which allows us to incorporate your work into the project. This is a one-time process for all your future contributions.

**How to sign the CLA:**

>> Click here to sign the Contributor License Agreement <<

We use an automated bot to check for a signed CLA on all pull requests. The bot will comment on your PR with a link to sign if you haven't done so already.

---

## Coding Style

Consistency is key. Please adhere to the following style guidelines:

- **[Language Name, e.g., Python]:** We follow the [Link to Style Guide](#) (e.g., PEP 8 style guide).
- **Linting:** Please run the linter before submitting your code.

	```sh
	# Linter command, e.g.,
	flake8 .
	```

- **Comments:** Use comments to explain complex logic, but prefer clear, self-documenting code where possible.

---

## Commit Message Guidelines

We follow a conventional commit format to maintain a clear and automated changelog. Please format your commit messages as follows:

```text
<type>(<scope>): <subject>

<body>
```

- **type:** `feat` (new feature), `fix` (bug fix), `docs` (documentation), `style` (formatting, white-space), `refactor`, `test`, `chore` (build process, etc.).
- **scope:** (optional) The part of the codebase you've changed (e.g., discovery, streaming, http-server).
- **subject:** A short, imperative-tense summary of the change (e.g., "Add support for FLAC transcoding").

**Example:**

```text
feat(renderer): add playback state reporting
```

---

## License

By contributing to VuIO, you agree that your contributions will be licensed under the Apache License 2.0. This is also covered by the CLA.

---

Thank you again for your interest in making VuIO better! We look forward to your contributions.