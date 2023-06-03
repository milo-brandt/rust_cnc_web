import { Box, CircularProgress } from "@mui/material";
import ErrorIcon from '@mui/icons-material/Error';

export function PageLoading({message = "Loading..."}: {message?: string}) {
  return (<Box display="flex" justifyContent="center" alignItems="center" width="100vw" height="90vh">
    <CircularProgress/> <Box sx={{ ml: 2 }}>{ message } </Box>
  </Box>);
}
export function PageErrored() {
  return (<Box display="flex" justifyContent="center" alignItems="center" width="100vw" height="90vh">
    <ErrorIcon sx={{ fontSize: 40 }}/> <Box sx={{ ml: 2 }}>Error</Box>
  </Box>);
}