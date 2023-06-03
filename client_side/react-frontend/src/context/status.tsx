import { type PropsWithChildren, createContext, useContext } from 'react'
import type { JobStatus, MachineStatus } from '../api/globalListener'
import type { Maybe } from '../util/types'

export interface TotalStatus {
  jobStatus: Maybe<JobStatus>
  machineStatus: Maybe<MachineStatus>
}

const StatusContext = createContext<TotalStatus | null>(null)

export function StatusProvider ({ value, children }: PropsWithChildren<{ value: TotalStatus }>) {
  return (
        <StatusContext.Provider value={value}>
          {children}
        </StatusContext.Provider>
  )
}

export function useStatusContext (): TotalStatus {
  const value = useContext(StatusContext)
  if (value === null) {
    throw Error('No StatusContext found! Perhaps you forgot to provide it?')
  } else {
    return value
  }
}
