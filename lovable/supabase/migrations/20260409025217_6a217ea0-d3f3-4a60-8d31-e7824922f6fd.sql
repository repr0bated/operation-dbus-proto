-- Fix privilege escalation: add explicit WITH CHECK to admin policy
DROP POLICY IF EXISTS "Admins can manage roles" ON user_roles;

CREATE POLICY "Admins can manage roles"
  ON user_roles FOR ALL
  TO authenticated
  USING (has_role(auth.uid(), 'admin'::app_role))
  WITH CHECK (has_role(auth.uid(), 'admin'::app_role));

-- Add restrictive INSERT policy as additional safeguard
CREATE POLICY "Only admins can insert roles"
  ON user_roles AS RESTRICTIVE FOR INSERT
  TO authenticated
  WITH CHECK (has_role(auth.uid(), 'admin'::app_role));

-- Fix services: restrict SELECT to operators/admins only (prevents cmdline credential leakage)
DROP POLICY IF EXISTS "Authenticated users can view services" ON services;

CREATE POLICY "Operators and admins can view services"
  ON services FOR SELECT
  TO authenticated
  USING (
    has_role(auth.uid(), 'admin'::app_role)
    OR has_role(auth.uid(), 'operator'::app_role)
  );