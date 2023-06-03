import { useMachineStatus, useStatus } from './api/globalListener'
import { Box, CircularProgress } from '@mui/material'
import { StatusProvider } from './context/status'
import { createBrowserRouter, RouterProvider } from 'react-router-dom'
import Layout from './Layout'
import { SnackBarProvider } from './context/snackbar'
import { DialogProvider } from './context/modal'
import { Suspense, createElement, lazy } from 'react'

const router = createBrowserRouter([
  {
    path: '/',
    element: <Layout/>,
    children: [
      {
        path: '/',
        element: createElement(lazy(() => import("./HomePage")))
      },
      {
        path: '/gcode/*',
        element: createElement(lazy(() => import("./FileList")))
      },
      {
        path: '/view/*',
        element: createElement(lazy(() => import("./ViewPage")))
      },
      {
        path: '/debug',
        element: createElement(lazy(() => import("./DebugPage")))
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
        <Suspense>
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
        </Suspense>
      </main>
    )
  }
}
