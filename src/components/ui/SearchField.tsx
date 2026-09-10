import { forwardRef, type InputHTMLAttributes } from 'react'

interface Props extends Omit<InputHTMLAttributes<HTMLInputElement>, 'onChange' | 'value'> {
  value: string
  onChange: (v: string) => void
}

/** Native-looking search field. Escape clears, then drops focus — as NSSearchField does. */
const SearchField = forwardRef<HTMLInputElement, Props>(function SearchField({ value, onChange, ...rest }, ref) {
  return (
    <input
      ref={ref}
      type="search"
      className="search"
      value={value}
      onChange={e => onChange(e.target.value)}
      onKeyDown={e => {
        if (e.key !== 'Escape') return
        if (value) onChange('')
        else (e.target as HTMLInputElement).blur()
      }}
      spellCheck={false}
      autoCorrect="off"
      autoCapitalize="off"
      {...rest}
    />
  )
})
export default SearchField
