import { AppBar, Divider, IconButton, Toolbar, Typography } from '@mui/material'
import { formatSeconds, useElapsedSeconds } from './util/time'
import { mapMaybe } from './util/types'
import PlayArrowIcon from '@mui/icons-material/PlayArrow'
import StopIcon from '@mui/icons-material/Stop'
import PauseIcon from '@mui/icons-material/Pause'
import HomeIcon from '@mui/icons-material/Home';
import { PowerSettingsNew } from '@mui/icons-material'
import * as immediateActions from './api/immediateActions'
import { useStatusContext } from './context/status'
import { Link } from 'react-router-dom'

export default function TaskBar () {
  const {
    jobStatus: status,
    machineStatus
  } = useStatusContext()
  // const status = useStatus();
  // const machineStatus = useMachineStatus();
  const elapsedSeconds = useElapsedSeconds(status?.startTime ?? null)
  const elapsedTimeString = mapMaybe(formatSeconds, elapsedSeconds)

  const showTimeControls = machineStatus?.state?.type !== 'Idle'
  const showPause = machineStatus?.state?.type !== 'Hold'

  return (
    <AppBar position="sticky" sx={{marginBottom: 1}}>
      <Toolbar>
      <IconButton
                size="large"
                edge="start"
                color="inherit"
                sx={{
                  visibility: showTimeControls ? 'visible' : 'hidden',
                }}
                component={ Link }
                to="/"
              >
            <HomeIcon/>
          </IconButton>
      <Divider orientation="vertical" light flexItem sx={{mr: 3, ml: 1, borderColor:"rgba(255,255,255,0.3)"}}/>
      <IconButton
                size="large"
                edge="start"
                color="inherit"
                sx={{
                  visibility: showTimeControls ? 'visible' : 'hidden'
                }}
                onClick={() => {
                  if (showPause) {
                    immediateActions.pause()
                  } else {
                    immediateActions.resume()
                  }
                }}
              >
            {
              showPause ? <PauseIcon /> : <PlayArrowIcon />
            }
          </IconButton>
          <IconButton
                size="large"
                edge="start"
                color="inherit"
                sx={{
                  visibility: showTimeControls ? 'visible' : 'hidden'
                }}
                onClick={async () => await immediateActions.reset()}
              >
            <StopIcon />
          </IconButton>
          <Typography variant='h6' sx={{ flexGrow: 1 }}> {
          status === null ? 'Idle' : `Running (${elapsedTimeString}) - ${status.message}`
        }</Typography>
        <IconButton
                        size="large"
                        edge="start"
                        color="inherit"
                        onClick={async () => await immediateActions.shutdown()}
        ><PowerSettingsNew/> </IconButton>
        </Toolbar>
    </AppBar>
  )
}
