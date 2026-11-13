# Finanças da Residência

Gestão compartilhada das finanças de uma casa: os moradores registram receitas e despesas e acompanham juntos a situação e a previsão do orçamento.

## Language

### Pessoas e grupos

**Residência**:
O grupo de pessoas cujas finanças são geridas em conjunto.
_Avoid_: Casa, família, grupo, household

**Usuário**:
Uma pessoa com credenciais de acesso, que pode pertencer a várias residências.
_Avoid_: Conta, login

**Membro**:
Uma pessoa dentro de uma residência específica, com papel de Administrador ou Comum. Na interface aparece como "participante". Pode não ter senha: nesse caso é só alguém a quem lançamentos são atribuídos, e não entra no app.
_Avoid_: Morador

**Administrador**:
Membro que pode vincular lançamentos a qualquer membro da residência; quem cria a residência é o primeiro.
_Avoid_: ADM, dono, host

**Comum**:
Membro cujos lançamentos ficam sempre vinculados a si mesmo.
_Avoid_: Usuário normal, convidado

**Convite**:
Um código de uso único e validade limitada, gerado por um Administrador, que permite a um usuário tornar-se membro da residência.
_Avoid_: Link, token, chave

### Lançamentos

**Lançamento**:
Um registro financeiro da residência, do tipo Receita ou Despesa, que está Previsto ou Efetivado.
_Avoid_: Transação, registro, movimentação

**Previsto**:
Situação de um lançamento que ainda não aconteceu.
_Avoid_: Pendente, futuro, agendado

**Efetivado**:
Situação de um lançamento que já aconteceu de fato.
_Avoid_: Pago, realizado, confirmado

**Categoria**:
Uma classificação de lançamentos de um mesmo tipo (Receita ou Despesa), pertencente à residência e mantida pelos Administradores.
_Avoid_: Tag, tipo, grupo

**Recorrência**:
Uma regra que gera lançamentos previstos periodicamente, como um salário ou um aluguel.
_Avoid_: Assinatura, lançamento fixo, repetição

**Parcelamento**:
Uma compra dividida em N despesas previstas em meses consecutivos.
_Avoid_: Prestação, financiamento

**Receita**:
Lançamento de entrada de dinheiro.
_Avoid_: Entrada, ganho

**Despesa**:
Lançamento de saída de dinheiro.
_Avoid_: Saída, gasto

**Responsável**:
O membro ao qual um lançamento está vinculado, que não é necessariamente quem o digitou.
_Avoid_: Dono, autor, criador

### Compartilhamento

Termos da sincronização entre computadores, engavetada (ADR 0003).

**Anfitrião**:
A única instância do app que hospeda uma residência, na máquina onde ela foi criada, e é a fonte da verdade dos seus dados.
_Avoid_: Server, host, ADM

**Cópia**:
Uma instância do app em outro computador, que guarda a residência inteira e a mantém em dia com o Anfitrião.
_Avoid_: Cliente, réplica, espelho

**Código de segurança**:
Um código curto, derivado da identidade do Anfitrião, exibido nos dois computadores durante o pareamento para confirmar que a Cópia está falando com o Anfitrião certo.
_Avoid_: PIN, senha do servidor

**Conflito**:
A situação em que o mesmo registro foi alterado em duas cópias diferentes entre duas sincronizações.
_Avoid_: Divergência
