package controlserver

import (
	"context"
	"fmt"
	"net/http"
	"sync"
	"time"

	"github.com/gin-gonic/gin"

	"github.com/woyin/OrangeCoding/modules/control-protocol"
	"github.com/woyin/OrangeCoding/modules/worker"
)

// eventSubscriber represents a WebSocket client's event subscription.
type eventSubscriber struct {
	ch     chan<- controlprotocol.ServerEvent
	cancel context.CancelFunc
}

// Server provides HTTP and WebSocket endpoints for the web-based control plane.
type Server struct {
	router      *gin.Engine
	workers     *worker.WorkerRuntime
	eventCh     chan controlprotocol.ServerEvent
	addr        string
	server      *http.Server
	subscribers map[*eventSubscriber]struct{}
	subMu       sync.RWMutex
}

// NewServer creates a new control server backed by the given WorkerRuntime.
func NewServer(workerRuntime *worker.WorkerRuntime, addr string) *Server {
	gin.SetMode(gin.ReleaseMode)
	router := gin.New()

	eventCh := make(chan controlprotocol.ServerEvent, 64)

	s := &Server{
		router:      router,
		workers:     workerRuntime,
		eventCh:     eventCh,
		addr:        addr,
		subscribers: make(map[*eventSubscriber]struct{}),
	}

	s.setupRoutes()
	go s.fanOutEvents()
	return s
}

// fanOutEvents reads from the shared eventCh and dispatches to all subscribers.
func (s *Server) fanOutEvents() {
	for event := range s.eventCh {
		s.subMu.RLock()
		subs := make([]*eventSubscriber, 0, len(s.subscribers))
		for sub := range s.subscribers {
			subs = append(subs, sub)
		}
		s.subMu.RUnlock()

		for _, sub := range subs {
			select {
			case sub.ch <- event:
			default:
				// Client is too slow; drop event for this subscriber.
			}
		}
	}
	// Channel closed; close all subscriber channels.
	s.subMu.RLock()
	for sub := range s.subscribers {
		close(sub.ch)
	}
	s.subMu.RUnlock()
}

// subscribeEvents registers a new event subscriber and returns an unsubscribe function.
func (s *Server) subscribeEvents(ch chan<- controlprotocol.ServerEvent) func() {
	sub := &eventSubscriber{ch: ch}
	s.subMu.Lock()
	s.subscribers[sub] = struct{}{}
	s.subMu.Unlock()
	return func() {
		s.subMu.Lock()
		delete(s.subscribers, sub)
		s.subMu.Unlock()
	}
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

// Stop performs a graceful shutdown with a 10-second timeout.
func (s *Server) Stop() error {
	// Cancel all running agent sessions first.
	s.workers.Shutdown()

	if s.server == nil {
		close(s.eventCh)
		return nil
	}
	ctx, cancel := context.WithTimeout(context.Background(), 10*time.Second)
	defer cancel()
	// Shutdown HTTP server first (waits for in-flight requests), then close eventCh.
	err := s.server.Shutdown(ctx)
	close(s.eventCh)
	return err
}

// Router returns the underlying gin.Engine (useful for testing with httptest).
func (s *Server) Router() *gin.Engine {
	return s.router
}
