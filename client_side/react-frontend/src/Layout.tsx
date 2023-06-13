import { Box } from "@mui/material";
import TaskBar from "./TaskBar";
import { Outlet } from "react-router-dom";

export default function Layout() {
  return (
    <Box height="100%" display="flex" flexDirection="column">
      <Box>
        <TaskBar/>
      </Box>
      <Box flexGrow={1}>
        <Outlet/>
      </Box>
    </Box>
  )
}