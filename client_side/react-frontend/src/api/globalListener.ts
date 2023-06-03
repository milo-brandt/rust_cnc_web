import { type Maybe, mapMaybe } from '../util/types'
import { useWebSocket } from './generic'

export interface JobStatus {
  startTime: Date
  message: string
}

export function useStatus () {
  return useWebSocket('/debug/listen_status', value => {
    return mapMaybe(data => {
      const {
        start_time,
        message
      } = data
      return {
        startTime: new Date(start_time),
        message
      }
    }, value)
  })
}

interface SimpleGrblStatus {
  type: 'Idle' | 'Run' | 'Jog' | 'Alarm' | 'Check' | 'Home' | 'Sleep'
}
interface CodedGrblStatus {
  type: 'Hold' | 'Door'
  code: number
}

export type GrblStatus = SimpleGrblStatus | CodedGrblStatus

export interface MachineStatus {
  state: GrblStatus
  machine_position: number[]
  work_coordinate_offset: number[]
  feed_override: number
  rapid_override: number
  spindle_override: number
  probe: boolean
}

export function useMachineStatus (): Maybe<MachineStatus> {
  return useWebSocket<MachineStatus>('/debug/listen_position', value => value as MachineStatus)
}
/*
export function useStatus(): Maybe<JobStatus> {
    const [status, setStatus] = useState<Maybe<JobStatus>>(null);
    useEffect(() => {
        const ws = new WebSocket(`ws://${HOST}/debug/listen_status`);
        ws.onmessage = (event) => {
            setStatus(mapMaybe(data => {
                const {
                    start_time,
                    message,
                } = data;
                return {
                    startTime: new Date(start_time),
                    message
                };
            }, JSON.parse(event.data as string) as Maybe<any>));
        };
        return () => {
            ws.close()
        }
    }, []);
    return status;
}
*/
