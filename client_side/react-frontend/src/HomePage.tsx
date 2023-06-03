import { List, ListItemButton, ListItemIcon, ListItemText, Typography } from "@mui/material";
import React from "react";

import {
  Link as RouterLink,
  LinkProps as RouterLinkProps,
} from 'react-router-dom';

interface ListItemLinkProps {
  icon?: React.ReactElement;
  primary: string;
  to: string;
}

const Link = React.forwardRef<HTMLAnchorElement, RouterLinkProps>(function Link(
  itemProps,
  ref,
) {
  return <RouterLink ref={ref} {...itemProps} role={undefined} />;
});

function ListItemLink(props: ListItemLinkProps) {
  const { icon, primary, to } = props;
  return (
    <li>
      <ListItemButton component={Link} to={to}>
        {icon ? <ListItemIcon>{icon}</ListItemIcon> : null}
        <ListItemText primary={primary} />
      </ListItemButton>
    </li>
  );
}

export default function HomePage() {
  return (
    <>
      <Typography variant="h4">Home</Typography>
      <List>
        <ListItemLink to="/gcode" primary="Files"/>
        <ListItemLink to="/debug" primary="Debug"/>
      </List>
    </>
  )
}