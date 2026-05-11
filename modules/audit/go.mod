module github.com/woyin/OrangeCoding/modules/audit

go 1.22

require go.etcd.io/bbolt v1.3.10

require (
	github.com/stretchr/testify v1.8.3 // indirect
	golang.org/x/sys v0.8.0 // indirect
)

replace github.com/woyin/OrangeCoding/modules/core => ../core
