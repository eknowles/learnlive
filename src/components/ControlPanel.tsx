import './ControlPanel.css'

interface Props {
  isListening: boolean
  status: string
  onStart: () => void
  onStop: () => void
  onClear: () => void
}

export default function ControlPanel({
  isListening,
  status,
  onStart,
  onStop,
  onClear,
}: Props) {
  return (
    <div className="control-panel">
      <div className="controls-left">
        <button
          className={`btn btn-primary ${isListening ? 'btn-danger' : 'btn-success'}`}
          onClick={isListening ? onStop : onStart}
        >
          <span className="btn-icon">
            {isListening ? '⏹' : '⏸'}
          </span>
          {isListening ? 'Stop Listening' : 'Start Listening'}
        </button>

        <button
          className="btn btn-secondary"
          onClick={onClear}
          disabled={isListening}
        >
          Clear History
        </button>
      </div>

      <div className="status-display">
        <div className={`status-indicator ${isListening ? 'status-active' : 'status-inactive'}`} />
        <span className="status-text">{status}</span>
      </div>

      <div className="controls-right">
        <div className="help-text">
          Connect to Google Meet • Enable BlackHole audio routing • Keep backend running
        </div>
      </div>
    </div>
  )
}
