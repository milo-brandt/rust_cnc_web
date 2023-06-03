import { useEffect, useState } from 'react'
import { type Maybe } from './types'

export function useElapsedSeconds (time: Maybe<Date>): Maybe<number> {
// Ensures 1 tick/second
  const [seconds, setSeconds] = useState<Maybe<number>>(null)
  const initialTime = time?.getTime()
  useEffect(() => {
    if (initialTime !== undefined) {
      let currentSeconds = Math.floor((new Date().getTime() - initialTime) / 1000)
      setSeconds(currentSeconds)
      const interval = setInterval(() => {
        currentSeconds += 1
        let actualSeconds = Math.floor((new Date().getTime() - initialTime) / 1000)
        // Tolerate differences of 1 second (to avoid issues near edge of floor)
        if (Math.abs(currentSeconds - actualSeconds) > 1) {
            currentSeconds = actualSeconds;
        }
        setSeconds(currentSeconds)
      }, 1000)
      return () => { clearInterval(interval) }
    } else {
      setSeconds(null)
    }
  }, [initialTime])
  return seconds
}

export function formatSeconds (elapsedSeconds: number): string {
  return new Date(elapsedSeconds * 1000).toISOString().substring(11, 19)
}
