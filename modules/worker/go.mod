module github.com/woyin/OrangeCoding/modules/worker

go 1.22

require (
	github.com/woyin/OrangeCoding/modules/agent v0.0.0
	github.com/woyin/OrangeCoding/modules/ai v0.0.0
	github.com/woyin/OrangeCoding/modules/control-protocol v0.0.0
	github.com/woyin/OrangeCoding/modules/core v0.0.0
)

require github.com/google/uuid v1.6.0 // indirect

replace (
	github.com/woyin/OrangeCoding/modules/agent => ../agent
	github.com/woyin/OrangeCoding/modules/ai => ../ai
	github.com/woyin/OrangeCoding/modules/control-protocol => ../control-protocol
	github.com/woyin/OrangeCoding/modules/core => ../core
)
