import React from "react";

const SayHello = ({ name }: { name: string }): JSX.Element => (
  <div>Hey {name}, say byue to TypeScript.</div>
);

export default SayHello;