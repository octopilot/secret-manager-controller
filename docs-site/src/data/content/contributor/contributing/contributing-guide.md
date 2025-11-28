# Contributing Guide

Thank you for your interest in contributing to the Secret Manager Controller! This guide will help you get started.

## Getting Started

1. **Fork and Clone**
   ```bash
   git clone https://github.com/microscaler/secret-manager-controller.git
   cd secret-manager-controller
   ```

2. **Set Up Development Environment**
   - Follow the [Development Setup](../development/setup.md) guide
   - Install Git hooks: `./scripts/install-git-hooks.sh`

3. **Create a Branch**
   ```bash
   git checkout -b feat/your-feature-name
   # or
   git checkout -b fix/your-bug-fix
   ```

## Development Workflow

### 1. Make Your Changes

- Write code following our [Code Style](../guidelines/code-style.md) guidelines
- Add tests for new features
- Update documentation as needed

### 2. Test Your Changes

```bash
# Run unit tests
cargo test

# Run with specific features
cargo test --features gcp,aws,azure

# Run integration tests (requires Kind cluster)
python3 scripts/setup_kind.py
cargo test --test integration
```

### 3. Commit Your Changes

**Important:** All commits must follow the [Conventional Commits](../guidelines/conventional-commits.md) specification.

**Commit Message Format:**
```
<type>[optional scope]: <description>

[optional body]

[optional footer(s)]
```

**Examples:**
```bash
# Feature
git commit -m "feat(aws): add Parameter Store support"

# Bug fix
git commit -m "fix(parser): handle empty secret files"

# Documentation
git commit -m "docs: update installation guide"

# With body
git commit -m "feat: add AGE encryption support

AGE (Actually Good Encryption) is a modern alternative to GPG
for SOPS encryption. This adds support for AGE keys alongside
existing GPG key support."
```

**Git Hook Validation:**
- A commit-msg hook automatically validates your commit messages
- Invalid messages will be rejected with helpful error messages
- See [Conventional Commits](../guidelines/conventional-commits.md) for complete details

### 4. Push and Create Pull Request

```bash
git push origin feat/your-feature-name
```

Then create a pull request on GitHub.

## Commit Guidelines

### Commit Types

- `feat`: A new feature
- `fix`: A bug fix
- `docs`: Documentation only changes
- `style`: Code style changes (formatting, etc.)
- `refactor`: Code refactoring
- `perf`: Performance improvements
- `test`: Adding or updating tests
- `build`: Build system changes
- `ci`: CI/CD changes
- `chore`: Maintenance tasks

### Scope

Use scopes to indicate the area of the codebase affected:
- `aws`, `gcp`, `azure` - Provider-specific
- `parser` - File parsing
- `reconciler` - Reconciliation logic
- `cli` - Command-line interface
- `docs` - Documentation
- `test` - Testing

### Breaking Changes

Use `!` after the type/scope to indicate breaking changes:

```bash
git commit -m "feat!: change CRD API version to v1

BREAKING CHANGE: The SecretManagerConfig CRD now uses v1 API version."
```

## Code Quality

### Pre-commit Checks

The pre-commit hook automatically runs:
- **SOPS encryption check**: Ensures all secret files are encrypted
- **Rust formatting**: Runs `cargo fmt` to format code
- **Rust checks**: Runs `cargo check` to verify compilation

### Code Style

- Follow Rust style guidelines (enforced by `cargo fmt`)
- See [Code Style](../guidelines/code-style.md) for detailed guidelines

### Error Handling

- Follow [Error Handling](../guidelines/error-handling.md) patterns
- Use appropriate error types and provide context

### Logging

- Follow [Logging](../guidelines/logging.md) guidelines
- Use appropriate log levels
- Include relevant context in log messages

## Testing

### Unit Tests

```bash
cargo test
```

### Integration Tests

Integration tests require a Kind cluster:

```bash
# Set up Kind cluster
python3 scripts/setup_kind.py

# Run integration tests
cargo test --test integration
```

### Pact Tests

Contract tests for provider APIs:

```bash
# Run Pact tests
cargo test --test pact_*
```

See [Testing Guide](../testing/testing-guide.md) for complete testing documentation.

## Documentation

### User Documentation

User-facing documentation is in `docs-site/src/data/content/user/`:
- Update relevant guides when adding features
- Add examples for new functionality
- Update API references for CRD changes

### Contributor Documentation

Contributor documentation is in `docs-site/src/data/content/contributor/`:
- Update development guides for workflow changes
- Document new testing patterns
- Update architecture docs for design changes

### Building Documentation

```bash
cd docs-site
yarn install
yarn build
```

## Pull Request Process

1. **Create a Branch**: Use a descriptive branch name
2. **Make Changes**: Follow coding guidelines and add tests
3. **Commit**: Use conventional commit messages
4. **Push**: Push your branch to your fork
5. **Create PR**: Open a pull request with a clear description

### PR Description Template

```markdown
## Description
Brief description of changes

## Type of Change
- [ ] Bug fix
- [ ] New feature
- [ ] Breaking change
- [ ] Documentation update

## Testing
- [ ] Unit tests added/updated
- [ ] Integration tests added/updated
- [ ] Manual testing performed

## Checklist
- [ ] Code follows style guidelines
- [ ] Self-review completed
- [ ] Comments added for complex code
- [ ] Documentation updated
- [ ] No new warnings generated
- [ ] Tests pass locally
```

## Code Review

- All PRs require review before merging
- Address review comments promptly
- Keep PRs focused and reasonably sized
- Rebase on main before requesting review

## Questions?

- Check the [Development Setup](../development/setup.md) guide
- Review [Conventional Commits](../guidelines/conventional-commits.md) for commit format
- See [Testing Guide](../testing/testing-guide.md) for testing help
- Open an issue for questions or discussions

## Next Steps

- [Development Setup](../development/setup.md) - Set up your environment
- [Conventional Commits](../guidelines/conventional-commits.md) - Commit message format
- [Code Style](../guidelines/code-style.md) - Coding guidelines
- [Testing Guide](../testing/testing-guide.md) - Testing strategies

