import { useState } from 'react'
import './TranslationDisplay.css'

interface WordAnalysis {
  word: string
  lemma: string
  pos: string
  case?: string
  tense?: string
  english_equiv?: string
}

interface TranslationData {
  id: string
  english: string
  russian: string
  analysis: WordAnalysis[]
  timestamp: number
}

interface Props {
  translations: TranslationData[]
}

export default function TranslationDisplay({ translations }: Props) {
  const [hoveredWord, setHoveredWord] = useState<string | null>(null)
  const [selectedWordId, setSelectedWordId] = useState<string | null>(null)

  const handleWordClick = async (word: WordAnalysis) => {
    setSelectedWordId(word.word)
    // TODO: Call backend to play audio of this word
    try {
      // await fetch(`http://127.0.0.1:8000/api/speak`, {
      //   method: 'POST',
      //   body: JSON.stringify({ text: word.word, language: 'ru' })
      // })
    } catch (error) {
      console.error('Failed to play audio:', error)
    }
  }

  const getPOSColor = (pos: string) => {
    const posMap: Record<string, string> = {
      'NOUN': '#3b82f6',
      'VERB': '#ef4444',
      'ADJ': '#22c55e',
      'ADV': '#eab308',
      'PREP': '#a855f7',
      'CONJ': '#d946ef',
      'PRON': '#06b6d4',
      'NUM': '#f59e0b',
    }
    return posMap[pos] || '#6b7280'
  }

  return (
    <div className="translation-display">
      {translations.map((translation) => (
        <div key={translation.id} className="translation-item">
          <div className="translation-pair">
            <div className="english-text">
              <strong>English:</strong>
              <p>{translation.english}</p>
            </div>

            <div className="russian-text">
              <strong>Russian:</strong>
              <div className="russian-words">
                {translation.analysis.map((wordAnalysis, idx) => (
                  <span
                    key={idx}
                    className={`word pos-${wordAnalysis.pos}`}
                    style={{
                      backgroundColor: `${getPOSColor(wordAnalysis.pos)}20`,
                      borderColor: getPOSColor(wordAnalysis.pos),
                    }}
                    onMouseEnter={() => setHoveredWord(wordAnalysis.word)}
                    onMouseLeave={() => setHoveredWord(null)}
                    onClick={() => handleWordClick(wordAnalysis)}
                  >
                    {wordAnalysis.word}
                    
                    {hoveredWord === wordAnalysis.word && (
                      <div className="word-analysis-popup">
                        <div className="popup-header">
                          <div className="word-lemma">{wordAnalysis.lemma}</div>
                          <span className="pos-badge" style={{
                            backgroundColor: getPOSColor(wordAnalysis.pos),
                            color: 'white'
                          }}>
                            {wordAnalysis.pos}
                          </span>
                        </div>
                        
                        {wordAnalysis.english_equiv && (
                          <div className="popup-section">
                            <strong>English:</strong>
                            <p>{wordAnalysis.english_equiv}</p>
                          </div>
                        )}

                        <div className="popup-section">
                          <strong>Grammar:</strong>
                          <ul>
                            {wordAnalysis.case && <li>Case: {wordAnalysis.case}</li>}
                            {wordAnalysis.tense && <li>Tense: {wordAnalysis.tense}</li>}
                            {!wordAnalysis.case && !wordAnalysis.tense && (
                              <li>Lemma: {wordAnalysis.lemma}</li>
                            )}
                          </ul>
                        </div>

                        <small className="popup-hint">Click to hear pronunciation</small>
                      </div>
                    )}
                  </span>
                ))}
              </div>
            </div>
          </div>

          <div className="timestamp">
            {new Date(translation.timestamp).toLocaleTimeString()}
          </div>
        </div>
      ))}
    </div>
  )
}
