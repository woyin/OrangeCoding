module github.com/woyin/OrangeCoding/modules/cli

go 1.22

require (
	github.com/spf13/cobra v1.8.0
	github.com/woyin/OrangeCoding/modules/config v0.0.0
	github.com/woyin/OrangeCoding/modules/control-protocol v0.0.0
	github.com/woyin/OrangeCoding/modules/control-server v0.0.0
	github.com/woyin/OrangeCoding/modules/worker v0.0.0
)

require (
	github.com/bytedance/sonic v1.9.1 // indirect
	github.com/chenzhuoyu/base64x v0.0.0-20221115062448-fe3a3abad311 // indirect
	github.com/gabriel-vasile/mimetype v1.4.2 // indirect
	github.com/gin-contrib/sse v0.1.0 // indirect
	github.com/gin-gonic/gin v1.9.1 // indirect
	github.com/go-playground/locales v0.14.1 // indirect
	github.com/go-playground/universal-translator v0.18.1 // indirect
	github.com/go-playground/validator/v10 v10.14.0 // indirect
	github.com/goccy/go-json v0.10.2 // indirect
	github.com/google/uuid v1.6.0 // indirect
	github.com/gorilla/websocket v1.5.3 // indirect
	github.com/inconshreveable/mousetrap v1.1.0 // indirect
	github.com/json-iterator/go v1.1.12 // indirect
	github.com/klauspost/cpuid/v2 v2.2.4 // indirect
	github.com/leodido/go-urn v1.2.4 // indirect
	github.com/mattn/go-isatty v0.0.19 // indirect
	github.com/modern-go/concurrent v0.0.0-20180306012644-bacd9c7ef1dd // indirect
	github.com/modern-go/reflect2 v1.0.2 // indirect
	github.com/pelletier/go-toml/v2 v2.0.8 // indirect
	github.com/spf13/pflag v1.0.5 // indirect
	github.com/twitchyliquid64/golang-asm v0.15.1 // indirect
	github.com/ugorji/go/codec v1.2.11 // indirect
	github.com/woyin/OrangeCoding/modules/agent v0.0.0 // indirect
	github.com/woyin/OrangeCoding/modules/ai v0.0.0 // indirect
	github.com/woyin/OrangeCoding/modules/core v0.0.0 // indirect
	github.com/woyin/OrangeCoding/modules/tools v0.0.0 // indirect
	golang.org/x/arch v0.3.0 // indirect
	golang.org/x/crypto v0.9.0 // indirect
	golang.org/x/net v0.10.0 // indirect
	golang.org/x/sys v0.8.0 // indirect
	golang.org/x/text v0.9.0 // indirect
	google.golang.org/protobuf v1.30.0 // indirect
	gopkg.in/yaml.v3 v3.0.1 // indirect
)

replace (
	github.com/woyin/OrangeCoding/modules/agent => ../agent
	github.com/woyin/OrangeCoding/modules/ai => ../ai
	github.com/woyin/OrangeCoding/modules/config => ../config
	github.com/woyin/OrangeCoding/modules/control-protocol => ../control-protocol
	github.com/woyin/OrangeCoding/modules/control-server => ../control-server
	github.com/woyin/OrangeCoding/modules/core => ../core
	github.com/woyin/OrangeCoding/modules/tools => ../tools
	github.com/woyin/OrangeCoding/modules/worker => ../worker
)
