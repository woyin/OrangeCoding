package tools

// PermissionDecision represents the outcome of a permission check.
type PermissionDecision int

const (
	DecisionAllow       PermissionDecision = iota // explicitly allowed
	DecisionDeny                                  // explicitly denied
	DecisionAsk                                   // ask the user
	DecisionAutoApprove                           // auto-approved
	DecisionConditional                           // allowed with conditions
)

// PermissionContext provides the information needed to make a permission decision.
type PermissionContext struct {
	WorkingDir string // current working directory
	FilePath   string // file being accessed (if applicable)
	Command    string // command being run (if applicable)
	IsReadOnly bool   // whether the operation is read-only
}
