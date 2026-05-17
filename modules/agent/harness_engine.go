package agent

import (
	"context"
	"fmt"
	"time"

	"github.com/woyin/OrangeCoding/modules/core"
)

// HarnessEngineConfig configures a harness run.
type HarnessEngineConfig struct {
	RunID           string
	SessionID       core.SessionId
	CheckpointStore CheckpointStore
}

// HarnessEngine owns the state-machine checkpoint for one run.
type HarnessEngine struct {
	config     HarnessEngineConfig
	checkpoint HarnessCheckpoint
}

// NewHarnessEngine creates a harness engine with an in-memory store by default.
func NewHarnessEngine(config HarnessEngineConfig) *HarnessEngine {
	if config.CheckpointStore == nil {
		config.CheckpointStore = NewMemoryCheckpointStore()
	}
	return &HarnessEngine{config: config}
}

// Start initializes the run and moves it to BuildContext.
func (e *HarnessEngine) Start(ctx context.Context, task string) (HarnessCheckpoint, error) {
	if e.config.RunID == "" {
		return HarnessCheckpoint{}, fmt.Errorf("harness engine: run id is required")
	}
	e.checkpoint = HarnessCheckpoint{
		RunID:     e.config.RunID,
		SessionID: e.config.SessionID,
		Task:      task,
		State:     HarnessStateInit,
		UpdatedAt: time.Now().UTC(),
	}
	return e.Transition(ctx, HarnessStateBuildContext, "start")
}

// Transition records a legal state transition and persists the checkpoint.
func (e *HarnessEngine) Transition(ctx context.Context, next HarnessState, reason string) (HarnessCheckpoint, error) {
	if err := ctx.Err(); err != nil {
		return HarnessCheckpoint{}, err
	}
	if e.checkpoint.RunID == "" {
		return HarnessCheckpoint{}, fmt.Errorf("harness engine: start must be called before transition")
	}
	if !isAllowedHarnessTransition(e.checkpoint.State, next) {
		return HarnessCheckpoint{}, fmt.Errorf("harness engine: illegal transition %s -> %s", e.checkpoint.State, next)
	}

	from := e.checkpoint.State
	e.checkpoint.State = next
	e.checkpoint.UpdatedAt = time.Now().UTC()
	e.checkpoint.Trace = append(e.checkpoint.Trace, HarnessTraceEvent{
		From:      from,
		To:        next,
		Reason:    reason,
		CreatedAt: time.Now().UTC(),
	})
	if next == HarnessStateCompleted {
		e.checkpoint.StopReason = StopReasonCompleted
	}
	if err := e.config.CheckpointStore.Save(ctx, e.checkpoint); err != nil {
		return HarnessCheckpoint{}, err
	}
	return cloneHarnessCheckpoint(e.checkpoint), nil
}

// Update mutates and persists the current checkpoint without changing state.
func (e *HarnessEngine) Update(ctx context.Context, mutate func(*HarnessCheckpoint)) (HarnessCheckpoint, error) {
	if err := ctx.Err(); err != nil {
		return HarnessCheckpoint{}, err
	}
	if e.checkpoint.RunID == "" {
		return HarnessCheckpoint{}, fmt.Errorf("harness engine: start must be called before update")
	}
	mutate(&e.checkpoint)
	e.checkpoint.UpdatedAt = time.Now().UTC()
	if err := e.config.CheckpointStore.Save(ctx, e.checkpoint); err != nil {
		return HarnessCheckpoint{}, err
	}
	return cloneHarnessCheckpoint(e.checkpoint), nil
}

func isAllowedHarnessTransition(from, to HarnessState) bool {
	if from == to {
		return true
	}
	allowed := map[HarnessState][]HarnessState{
		HarnessStateInit:           {HarnessStateBuildContext, HarnessStateFailed},
		HarnessStateBuildContext:   {HarnessStateModelCall, HarnessStateStopped, HarnessStateFailed},
		HarnessStateModelCall:      {HarnessStateGuardrailCheck, HarnessStateFailed},
		HarnessStateGuardrailCheck: {HarnessStateToolDispatch, HarnessStateCompleted, HarnessStateStopped, HarnessStateFailed},
		HarnessStateToolDispatch:   {HarnessStateObserve, HarnessStateFailed},
		HarnessStateObserve:        {HarnessStateMemoryUpdate, HarnessStateFailed},
		HarnessStateMemoryUpdate:   {HarnessStateCheckpoint, HarnessStateFailed},
		HarnessStateCheckpoint:     {HarnessStateDecideNext, HarnessStateFailed},
		HarnessStateDecideNext:     {HarnessStateBuildContext, HarnessStateCompleted, HarnessStateStopped, HarnessStateFailed},
		HarnessStateCompleted:      {},
		HarnessStateStopped:        {},
		HarnessStateFailed:         {},
	}
	for _, candidate := range allowed[from] {
		if candidate == to {
			return true
		}
	}
	return false
}
