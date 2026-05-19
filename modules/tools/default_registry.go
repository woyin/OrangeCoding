package tools

// CreateDefaultRegistry creates a ToolRegistry with all built-in tools registered.
func CreateDefaultRegistry() *ToolRegistry {
	r := NewToolRegistry()

	// Shell
	r.Register(NewBashTool(DefaultSecurityPolicy()))

	// File operations
	r.Register(NewReadFileTool())
	r.Register(NewWriteFileTool())
	r.Register(NewEditFileTool())
	r.Register(NewDeleteFileTool())
	r.Register(NewListDirectoryTool())

	// Search
	r.Register(NewGrepTool())
	r.Register(NewFindTool())
	r.Register(NewGlobTool())

	// Network
	r.Register(NewFetchTool())

	// Language runtimes
	r.Register(NewPythonTool())

	// Utility
	r.Register(NewCalcTool())
	r.Register(NewTaskTool())

	// Stubs (not yet implemented)
	r.Register(NewBrowserTool())
	r.Register(NewSshTool())
	r.Register(NewLspTool())
	r.Register(NewWebSearchTool())
	r.Register(NewNotebookTool())

	return r
}
