# Agent Development Guidelines

## Core Principles

1. **Do NOT jump ahead** - Never implement infrastructure or systems that have not been explicitly asked for or discussed in a PRD
2. **Follow established patterns** - All code must conform to `./tasks/rust-guidelines.txt`
3. **Ask before adding** - Always request confirmation before adding new infrastructure components
4. **Documentation location** - Any markdown documentation created by the agent MUST go in `./agent/docs/`. The `./docs/` directory is reserved for human-created documentation only
5. **Component-specific documentation** - Always check for `CONTRIBUTING.md` files in component directories (`./ui/*/CONTRIBUTING.md` or `./components/*/CONTRIBUTING.md`) before making changes. These files contain component-specific guidelines, patterns, and configuration details.

## Technology Standards

### Infrastructure Management
- **FluxCD** with Helm releases for all infrastructure
- After adding Helm releases, they MUST be committed to allow Flux to reconcile from the remote repository
- Kind cluster for local Kubernetes development

### Development Tools
- **Justfile** for task automation
- **Tiltfile** for development environment orchestration
- **k3s** for local Kubernetes clusters

## Development Workflow

### Before Implementation
1. Check if the feature/infrastructure is documented in a PRD
2. If not in PRD, ask for confirmation before proceeding
3. Review `./tasks/rust-guidelines.txt` for implementation patterns
4. **Check component-specific documentation**:
   - Look for `CONTRIBUTING.md` in the target component directory
   - For UI components: Check `./ui/*/CONTRIBUTING.md` (e.g., `./ui/website/CONTRIBUTING.md`)
   - For Rust components: Check `./components/*/CONTRIBUTING.md` (if exists)
   - Review component-specific patterns, build configurations, and guidelines

### During Implementation
1. Follow Rust guidelines strictly
2. Use established tools and patterns
3. Keep implementations minimal and focused on requirements

### After Implementation
1. Commit Helm releases for Flux reconciliation
2. Verify Flux has reconciled from remote repository
3. Document any deviations or decisions made in `./agent/docs/`

## Infrastructure Checklist

Before adding any new infrastructure component:
- [ ] Is it specified in the PRD?
- [ ] Have you asked for confirmation?
- [ ] Is there a Helm chart available?
- [ ] Will it be managed by Flux?
- [ ] Have you prepared the commit for Flux reconciliation?

## Common Pitfalls to Avoid

1. **Overengineering** - Don't add services "just in case"
2. **Skipping confirmation** - Always ask before adding infrastructure
3. **Ignoring patterns** - Follow rust-guidelines.txt without exception
4. **Forgetting Flux** - Remember to commit Helm releases for reconciliation
5. **Tool proliferation** - Stick to the approved tool stack
6. **Wrong documentation location** - Never create markdown files in `./docs/` - use `./agent/docs/` instead
7. **Ignoring component docs** - Always check `CONTRIBUTING.md` files in component directories for component-specific guidelines

## Approved Tool Stack

| Category | Tool | Purpose |
|----------|------|---------|
| Orchestration | FluxCD | GitOps infrastructure management |
| Package Management | Helm | Kubernetes package management |
| Task Runner | Just | Command automation |
| Dev Environment | Tilt | Development orchestration |
| Local K8s | Kind | Local Kubernetes clusters |


## Questions to Ask

When uncertain about implementation:
1. "Is this infrastructure component specified in the PRD?"
2. "Should I proceed with adding [component]?"
3. "Is there a preferred Helm chart for [service]?"
4. "Should this be managed by Flux or deployed differently?"
5. "Is there a `CONTRIBUTING.md` file for this component that I should review?"

## Remember

**Less is more** - Start with the minimum viable implementation and only add complexity when explicitly requested.

---
