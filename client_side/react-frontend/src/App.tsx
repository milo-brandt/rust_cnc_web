import { useMachineStatus, useStatus } from './api/globalListener'
import { Box, CircularProgress } from '@mui/material'
import { StatusProvider } from './context/status'
import { createBrowserRouter, RouterProvider } from 'react-router-dom'
import HomePage from './HomePage'
import { Layout } from './Layout'
import { FileListPage } from './FileList'
import { SnackBarProvider } from './context/snackbar'
import { DialogProvider } from './context/modal'
import DebugPage from './DebugPage'
import { ViewPage } from './ViewPage'

const router = createBrowserRouter([
  {
    path: '/',
    element: <Layout/>,
    children: [
      {
        path: '/',
        element: <HomePage/>,
      },
      {
        path: '/gcode/*',
        element: <FileListPage/>
      },
      {
        path: '/view/*',
        element: <ViewPage/>
      },
      {
        path: '/debug',
        element: <DebugPage/>
      }
    ]
  },
])

export default function Home () {
  const status = useStatus()
  const machineStatus = useMachineStatus()
  if (machineStatus === null) {
    return (
      <main>
        <Box display="flex" justifyContent="center" alignItems="center" width="100vw" height="100vh">
            <CircularProgress/> <Box sx={{ ml: 2 }}>Connecting...</Box>
        </Box>
      </main>
    )
  } else {
    return (
      <main>
        <Box margin="1rem">
          <StatusProvider value={{
            jobStatus: status,
            machineStatus
          }}>
            <SnackBarProvider>
              <DialogProvider>
                <RouterProvider router={router}/>
              </DialogProvider>
            </SnackBarProvider>
          </StatusProvider>
        </Box>
      </main>
    )
  }
}
