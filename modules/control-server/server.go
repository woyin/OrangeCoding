package controlserver

import (
	"context"
	"fmt"
	"net/http"

	"github.com/gin-gonic/gin"

	"github.com/woyin/OrangeCoding/modules/control-protocol"
	"github.com/woyin/OrangeCoding/modules/worker"
)

// Server provides HTTP and WebSocket endpoints for the web-based control plane.
type Server struct {
	router  *gin.Engine
	workers *worker.WorkerRuntime
	eventCh chan controlprotocol.ServerEvent
	addr    string
	server  *http.Server
}

// NewServer creates a new control server backed by the given WorkerRuntime.
func NewServer(workerRuntime *worker.WorkerRuntime, addr string) *Server {
	gin.SetMode(gin.ReleaseMode)
	router := gin.New()

	eventCh := make(chan controlprotocol.ServerEvent, 64)

	s := &Server{
		router:  router,
		workers: workerRuntime,
		eventCh: eventCh,
		addr:    addr,
	}

	s.setupRoutes()
	return s
}

// setupRoutes registers all HTTP and WebSocket routes.
func (s *Server) setupRoutes() {
	s.router.Use(corsMiddleware())
	s.router.Use(loggingMiddleware())
	s.router.Use(gin.Recovery())

	api := s.router.Group("/")
	{
		api.POST("/sessions", s.createSession)
		api.GET("/sessions", s.listSessions)
		api.GET("/sessions/:id", s.getSession)
		api.POST("/sessions/:id/task", s.sendTask)
		api.DELETE("/sessions/:id", s.cancelSession)
		api.GET("/status", s.status)
		api.GET("/ws", s.handleWebSocket)
	}
}

// Start begins listening on the configured address.
func (s *Server) Start() error {
	s.server = &http.Server{
		Addr:    s.addr,
		Handler: s.router,
	}

	errCh := make(chan error, 1)
	go func() {
		if err := s.server.ListenAndServe(); err != nil && err != http.ErrServerClosed {
			errCh <- fmt.Errorf("server listen: %w", err)
		}
		close(errCh)
	}()

	// Wait briefly to catch immediate startup errors
	select {
	case err := <-errCh:
		return err
	default:
		return nil
	}
}

// Stop performs a graceful shutdown.
func (s *Server) Stop() error {
	if s.server == nil {
		return nil
	}
	return s.server.Shutdown(context.Background())
}

// Router returns the underlying gin.Engine (useful for testing with httptest).
func (s *Server) Router() *gin.Engine {
	return s.router
}
