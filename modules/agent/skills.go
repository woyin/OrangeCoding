package agent

// Skill defines a named capability with associated tools and a system prompt.
type Skill struct {
	Name        string
	Description string
	Tools       []string
	Prompt      string
}

// SkillRegistry manages available skills, including built-in skills.
type SkillRegistry struct {
	skills map[string]Skill
}

// NewSkillRegistry creates a new SkillRegistry with 6 built-in skills registered.
func NewSkillRegistry() *SkillRegistry {
	r := &SkillRegistry{
		skills: make(map[string]Skill),
	}

	r.registerBuiltins()
	return r
}

// Register adds a skill to the registry.
func (r *SkillRegistry) Register(s Skill) {
	r.skills[s.Name] = s
}

// Get retrieves a skill by name. Returns the skill and true if found.
func (r *SkillRegistry) Get(name string) (Skill, bool) {
	s, ok := r.skills[name]
	return s, ok
}

// List returns all registered skills.
func (r *SkillRegistry) List() []Skill {
	out := make([]Skill, 0, len(r.skills))
	for _, s := range r.skills {
		out = append(out, s)
	}
	return out
}

// registerBuiltins registers the 6 built-in skills.
func (r *SkillRegistry) registerBuiltins() {
	builtins := []Skill{
		{
			Name:        "code",
			Description: "Code implementation and modification",
			Tools:       []string{"bash", "read_file", "write_file", "edit_file"},
			Prompt:      "You are a code implementation agent. Write, modify, and debug code efficiently.",
		},
		{
			Name:        "debug",
			Description: "Debugging and error diagnosis",
			Tools:       []string{"bash", "read_file", "grep"},
			Prompt:      "You are a debugging agent. Diagnose and fix errors systematically.",
		},
		{
			Name:        "review",
			Description: "Code review and quality analysis",
			Tools:       []string{"read_file", "grep", "find"},
			Prompt:      "You are a code review agent. Analyze code quality and suggest improvements.",
		},
		{
			Name:        "plan",
			Description: "Task planning and decomposition",
			Tools:       []string{"read_file", "find", "grep"},
			Prompt:      "You are a planning agent. Break down complex tasks into actionable steps.",
		},
		{
			Name:        "explore",
			Description: "Codebase exploration and understanding",
			Tools:       []string{"read_file", "find", "grep", "glob"},
			Prompt:      "You are an exploration agent. Navigate and understand codebase structure.",
		},
		{
			Name:        "refactor",
			Description: "Code refactoring and cleanup",
			Tools:       []string{"bash", "read_file", "write_file", "edit_file", "grep"},
			Prompt:      "You are a refactoring agent. Improve code structure while preserving behavior.",
		},
	}

	for _, s := range builtins {
		r.Register(s)
	}
}
